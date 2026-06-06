use std::net::IpAddr;
use std::sync::Mutex;

use bunny_net_api::core::{
    AddDnsRecord, CoreClient, DnsRecord, DnsRecordType, DnsZone, UpdateDnsRecord,
};

use crate::model::{CaaRecord, Desire};
use crate::provider::Provider;

pub struct BunnyProvider {
    client: CoreClient,
    zone_id: i64,
    zone: Mutex<Option<DnsZone>>,
}

enum RecordResolution {
    Match,
    Missing,
    Mismatch(Box<DnsRecord>),
}

impl BunnyProvider {
    pub fn new(api_key: impl Into<String>, zone_id: i64) -> Self {
        Self {
            client: CoreClient::new(api_key.into()),
            zone_id,
            zone: Mutex::new(None),
        }
    }

    async fn zone(&self) -> Result<DnsZone, String> {
        if let Some(zone) = self.zone.lock().expect("zone mutex poisoned").clone() {
            return Ok(zone);
        }

        let zone = self
            .client
            .get_dns_zone(self.zone_id)
            .await
            .map_err(|e| format!("failed to fetch DNS zone {}: {e}", self.zone_id))?;

        *self.zone.lock().expect("zone mutex poisoned") = Some(zone.clone());
        Ok(zone)
    }

    fn invalidate_zone_cache(&self) {
        *self.zone.lock().expect("zone mutex poisoned") = None;
    }

    fn is_apex_record(record: &DnsRecord) -> bool {
        record.name.is_empty() || record.name == "@"
    }

    fn normalize_dns_name(value: &str) -> &str {
        value.trim_end_matches('.')
    }

    async fn has_caa(&self, caa_record: &CaaRecord) -> Result<bool, String> {
        let zone = self.zone().await?;

        Ok(zone
            .records
            .into_iter()
            .filter(Self::is_apex_record)
            .any(|record| {
                record.record_type == Some(DnsRecordType::CAA)
                    && record.tag.as_ref().is_some_and(|tag| {
                        CaaRecord::parse_dns_value(tag, &record.value)
                            .is_some_and(|parsed| parsed == *caa_record)
                    })
            }))
    }

    async fn ensure_caa(&self, caa_record: &CaaRecord) -> Result<(), String> {
        if self.has_caa(caa_record).await? {
            return Ok(());
        }

        let (tag, value) = caa_record.to_dns_value();
        let req = AddDnsRecord::new(DnsRecordType::CAA, value)
            .tag(&tag)
            .name("@");
        self.client
            .add_dns_record(self.zone_id, &req)
            .await
            .map_err(|e| {
                format!(
                    "failed to add root CAA record in zone {}: {e}",
                    self.zone_id
                )
            })?;

        self.invalidate_zone_cache();
        Ok(())
    }

    fn resolve_ip_record(zone: &DnsZone, value: IpAddr) -> RecordResolution {
        let (record_type, expected_value) = match value {
            IpAddr::V4(v4) => (DnsRecordType::A, v4.to_string()),
            IpAddr::V6(v6) => (DnsRecordType::AAAA, v6.to_string()),
        };

        if let Some(existing) = zone
            .records
            .iter()
            .find(|record| Self::is_apex_record(record) && record.record_type == Some(record_type))
        {
            if existing.value == expected_value {
                RecordResolution::Match
            } else {
                RecordResolution::Mismatch(Box::new(existing.clone()))
            }
        } else {
            RecordResolution::Missing
        }
    }

    async fn has_ip(&self, expected: IpAddr) -> Result<bool, String> {
        let zone = self.zone().await?;

        Ok(matches!(
            Self::resolve_ip_record(&zone, expected),
            RecordResolution::Match
        ))
    }

    async fn upsert_ip(&self, value: IpAddr) -> Result<(), String> {
        let zone = self.zone().await?;
        let record_type = match value {
            IpAddr::V4(_) => DnsRecordType::A,
            IpAddr::V6(_) => DnsRecordType::AAAA,
        };

        match Self::resolve_ip_record(&zone, value) {
            RecordResolution::Match => return Ok(()),
            RecordResolution::Mismatch(existing) => {
                let req =
                    UpdateDnsRecord::new(existing.id, record_type, value.to_string()).name("@");

                self.client
                    .update_dns_record(self.zone_id, existing.id, &req)
                    .await
                    .map_err(|e| {
                        format!(
                            "failed to update {:?} root record in zone {}: {e}",
                            record_type, self.zone_id
                        )
                    })?;
            }
            RecordResolution::Missing => {
                let req = AddDnsRecord::new(record_type, value.to_string()).name("@");
                self.client
                    .add_dns_record(self.zone_id, &req)
                    .await
                    .map_err(|e| {
                        format!(
                            "failed to add {:?} root record in zone {}: {e}",
                            record_type, self.zone_id
                        )
                    })?;
            }
        }

        self.invalidate_zone_cache();
        Ok(())
    }

    async fn has_txt_content(&self, expected: &str) -> Result<bool, String> {
        let zone = self.zone().await?;

        Ok(zone
            .records
            .into_iter()
            .filter(Self::is_apex_record)
            .any(|record| {
                record.record_type == Some(DnsRecordType::TXT) && record.value == expected
            }))
    }

    async fn ensure_txt(&self, content: &str) -> Result<(), String> {
        if self.has_txt_content(content).await? {
            return Ok(());
        }

        let req = AddDnsRecord::new(DnsRecordType::TXT, content).name("@");
        self.client
            .add_dns_record(self.zone_id, &req)
            .await
            .map_err(|e| {
                format!(
                    "failed to add root TXT record in zone {}: {e}",
                    self.zone_id
                )
            })?;

        self.invalidate_zone_cache();
        Ok(())
    }

    fn resolve_subdomain_cname_record(zone: &DnsZone, name: &str) -> RecordResolution {
        let expected_target = Self::normalize_dns_name(&zone.domain);

        if let Some(existing) = zone
            .records
            .iter()
            .find(|record| record.name == name && record.record_type == Some(DnsRecordType::CNAME))
        {
            if Self::normalize_dns_name(&existing.value) == expected_target {
                RecordResolution::Match
            } else {
                RecordResolution::Mismatch(Box::new(existing.clone()))
            }
        } else {
            RecordResolution::Missing
        }
    }

    async fn has_subdomain_cname(&self, name: &str) -> Result<bool, String> {
        let zone = self.zone().await?;

        Ok(matches!(
            Self::resolve_subdomain_cname_record(&zone, name),
            RecordResolution::Match
        ))
    }

    async fn upsert_subdomain_cname(&self, name: &str) -> Result<(), String> {
        let zone = self.zone().await?;
        let expected_target = Self::normalize_dns_name(&zone.domain);

        match Self::resolve_subdomain_cname_record(&zone, name) {
            RecordResolution::Match => return Ok(()),
            RecordResolution::Mismatch(existing) => {
                let req = UpdateDnsRecord::new(existing.id, DnsRecordType::CNAME, expected_target)
                    .name(name.to_owned());
                self.client
                    .update_dns_record(self.zone_id, existing.id, &req)
                    .await
                    .map_err(|e| {
                        format!(
                            "failed to update CNAME record {name} in zone {}: {e}",
                            self.zone_id
                        )
                    })?;
            }
            RecordResolution::Missing => {
                let req =
                    AddDnsRecord::new(DnsRecordType::CNAME, expected_target).name(name.to_owned());
                self.client
                    .add_dns_record(self.zone_id, &req)
                    .await
                    .map_err(|e| {
                        format!(
                            "failed to add CNAME record {name} in zone {}: {e}",
                            self.zone_id
                        )
                    })?;
            }
        }

        self.invalidate_zone_cache();
        Ok(())
    }
}

impl Provider for BunnyProvider {
    async fn evaluate(&self, desire: &Desire) -> Result<(), String> {
        match desire {
            Desire::Subdomain { name } => {
                if self.has_subdomain_cname(name).await? {
                    Ok(())
                } else {
                    Err(format!(
                        "Bunny zone {} does not contain subdomain {} as a CNAME to the zone apex",
                        self.zone_id, name
                    ))
                }
            }
            Desire::Address { value } => {
                if self.has_ip(*value).await? {
                    Ok(())
                } else {
                    Err(format!(
                        "Bunny zone {} apex record does not contain desired address {value}",
                        self.zone_id
                    ))
                }
            }
            Desire::Txt { content } => {
                if self.has_txt_content(content).await? {
                    Ok(())
                } else {
                    Err(format!(
                        "Bunny zone {} apex TXT records do not contain desired content {:?}",
                        self.zone_id, content
                    ))
                }
            }
            Desire::Caa(caa_record) => {
                if self.has_caa(caa_record).await? {
                    Ok(())
                } else {
                    Err(format!(
                        "Bunny zone {} apex CAA records do not contain desired record {}",
                        self.zone_id, caa_record
                    ))
                }
            }
        }
    }

    async fn apply(&self, desire: &Desire) -> Result<(), String> {
        match desire {
            Desire::Subdomain { name } => self.upsert_subdomain_cname(name).await,
            Desire::Address { value } => self.upsert_ip(*value).await,
            Desire::Txt { content } => self.ensure_txt(content).await,
            Desire::Caa(caa_record) => self.ensure_caa(caa_record).await,
        }
    }
}
