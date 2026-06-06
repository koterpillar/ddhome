pub mod auth;

use std::net::IpAddr;
use std::sync::Mutex;

use bunny_net_api::core::{
    AddDnsRecord, CoreClient, DnsRecord, DnsRecordType, DnsZone, UpdateDnsRecord,
};

use crate::model::{CaaRecord, Desire};
use crate::providers::Provider;
use crate::providers::util::RecordResolution;

pub use auth::read_bunny_api_key;

pub struct BunnyProvider {
    client: CoreClient,
    zone_id: i64,
    zone: Mutex<Option<DnsZone>>,
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

    async fn resolve_ip_record(
        &self,
        value: IpAddr,
    ) -> Result<RecordResolution<DnsRecord>, String> {
        let (record_type, expected_value) = match value {
            IpAddr::V4(v4) => (DnsRecordType::A, v4.to_string()),
            IpAddr::V6(v6) => (DnsRecordType::AAAA, v6.to_string()),
        };

        let zone = self.zone().await?;

        Ok(RecordResolution::from_expected(
            zone.records.into_iter().find(|record| {
                Self::is_apex_record(record) && record.record_type == Some(record_type)
            }),
            |actual: &DnsRecord| actual.value == expected_value,
        ))
    }

    async fn has_ip(&self, expected: IpAddr) -> Result<bool, String> {
        Ok(self.resolve_ip_record(expected).await?.is_match())
    }

    async fn upsert_ip(&self, value: IpAddr) -> Result<(), String> {
        let record_type = match value {
            IpAddr::V4(_) => DnsRecordType::A,
            IpAddr::V6(_) => DnsRecordType::AAAA,
        };

        self.resolve_ip_record(value)
            .await?
            .correct(Ok(()), async |existing| match existing {
                None => {
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
                    Ok(())
                }
                Some(existing) => {
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
                        })
                }
            })
            .await?;

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

    async fn domain(&self) -> Result<String, String> {
        let zone = self.zone().await?;
        let result = Self::normalize_dns_name(&zone.domain);
        Ok(result.to_owned())
    }

    async fn resolve_subdomain_cname_record(
        &self,
        name: &str,
    ) -> Result<RecordResolution<DnsRecord>, String> {
        let target = self.domain().await?;
        Ok(RecordResolution::from_expected(
            self.zone().await?.records.into_iter().find(|record| {
                record.name == name && record.record_type == Some(DnsRecordType::CNAME)
            }),
            |actual: &DnsRecord| Self::normalize_dns_name(&actual.value) == target,
        ))
    }

    async fn has_subdomain_cname(&self, name: &str) -> Result<bool, String> {
        Ok(self.resolve_subdomain_cname_record(name).await?.is_match())
    }

    async fn upsert_subdomain_cname(&self, name: &str) -> Result<(), String> {
        let target = self.domain().await?;

        self.resolve_subdomain_cname_record(name)
            .await?
            .correct(Ok(()), async |existing| match existing {
                None => {
                    let req = AddDnsRecord::new(DnsRecordType::CNAME, target).name(name.to_owned());
                    self.client
                        .add_dns_record(self.zone_id, &req)
                        .await
                        .map_err(|e| {
                            format!(
                                "failed to add CNAME record {name} in zone {}: {e}",
                                self.zone_id
                            )
                        })?;
                    Ok(())
                }
                Some(existing) => {
                    let req = UpdateDnsRecord::new(existing.id, DnsRecordType::CNAME, target)
                        .name(name.to_owned());
                    self.client
                        .update_dns_record(self.zone_id, existing.id, &req)
                        .await
                        .map_err(|e| {
                            format!(
                                "failed to update CNAME record {name} in zone {}: {e}",
                                self.zone_id
                            )
                        })
                }
            })
            .await?;

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
