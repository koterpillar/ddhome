use std::net::IpAddr;
use std::sync::Mutex;

use bunny_net_api::core::{
    AddDnsRecord, CoreClient, DnsRecord, DnsRecordType, DnsZone, UpdateDnsRecord,
};

use crate::model::Desire;
use crate::provider::Provider;

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

    fn is_root_record(record: &DnsRecord) -> bool {
        record.name.is_empty() || record.name == "@"
    }

    fn normalize_dns_name(value: &str) -> &str {
        value.trim_end_matches('.')
    }

    async fn has_root_ip(&self, expected: IpAddr) -> Result<bool, String> {
        let zone = self.zone().await?;

        Ok(zone
            .records
            .into_iter()
            .filter(Self::is_root_record)
            .any(|record| match expected {
                IpAddr::V4(expected_v4) => {
                    if record.record_type != Some(DnsRecordType::A) {
                        return false;
                    }

                    if let Ok(actual_v4) = record.value.parse::<std::net::Ipv4Addr>() {
                        return actual_v4 == expected_v4;
                    }

                    false
                }
                IpAddr::V6(expected_v6) => {
                    if record.record_type != Some(DnsRecordType::AAAA) {
                        return false;
                    }

                    if let Ok(actual_v6) = record.value.parse::<std::net::Ipv6Addr>() {
                        return actual_v6 == expected_v6;
                    }

                    false
                }
            }))
    }

    async fn has_root_txt_content(&self, expected: &str) -> Result<bool, String> {
        let zone = self.zone().await?;

        Ok(zone
            .records
            .into_iter()
            .filter(Self::is_root_record)
            .any(|record| {
                record.record_type == Some(DnsRecordType::TXT) && record.value == expected
            }))
    }

    async fn has_subdomain_cname_to_root(&self, name: &str) -> Result<bool, String> {
        let zone = self.zone().await?;
        let root_domain = zone.domain;
        let expected_target = Self::normalize_dns_name(&root_domain);

        Ok(zone.records.into_iter().any(|record| {
            if record.record_type != Some(DnsRecordType::CNAME) {
                return false;
            }

            if record.name != name {
                return false;
            }

            Self::normalize_dns_name(&record.value) == expected_target
        }))
    }

    async fn upsert_root_ip(&self, value: IpAddr) -> Result<(), String> {
        let zone = self.zone().await?;
        let (record_type, expected_value) = match value {
            IpAddr::V4(v4) => (DnsRecordType::A, v4.to_string()),
            IpAddr::V6(v6) => (DnsRecordType::AAAA, v6.to_string()),
        };

        if let Some(existing) = zone
            .records
            .iter()
            .find(|record| Self::is_root_record(record) && record.record_type == Some(record_type))
        {
            if existing.value == expected_value {
                return Ok(());
            }

            let mut req = UpdateDnsRecord::new(existing.id, record_type, expected_value.clone());
            if !existing.name.is_empty() {
                req = req.name(existing.name.clone());
            }

            self.client
                .update_dns_record(self.zone_id, existing.id, &req)
                .await
                .map_err(|e| {
                    format!(
                        "failed to update {:?} root record in zone {}: {e}",
                        record_type, self.zone_id
                    )
                })?;
        } else {
            let req = AddDnsRecord::new(record_type, expected_value).name("@");
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

        self.invalidate_zone_cache();
        Ok(())
    }

    async fn ensure_root_txt(&self, content: &str) -> Result<(), String> {
        if self.has_root_txt_content(content).await? {
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

    async fn upsert_subdomain_cname_to_root(&self, name: &str) -> Result<(), String> {
        let zone = self.zone().await?;
        let expected_target = Self::normalize_dns_name(&zone.domain).to_owned();

        if let Some(existing) = zone
            .records
            .iter()
            .find(|record| record.name == name && record.record_type == Some(DnsRecordType::CNAME))
        {
            if Self::normalize_dns_name(&existing.value) == expected_target {
                return Ok(());
            }

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
        } else {
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

        self.invalidate_zone_cache();
        Ok(())
    }
}

impl Provider for BunnyProvider {
    async fn evaluate(&self, desire: &Desire) -> Result<(), String> {
        match desire {
            Desire::Subdomain { name } => {
                if self.has_subdomain_cname_to_root(name).await? {
                    Ok(())
                } else {
                    Err(format!(
                        "Bunny zone {} does not contain subdomain {} as a CNAME to the zone root",
                        self.zone_id, name
                    ))
                }
            }
            Desire::Address { value } => {
                if self.has_root_ip(*value).await? {
                    Ok(())
                } else {
                    Err(format!(
                        "Bunny zone {} root record does not contain desired address {value}",
                        self.zone_id
                    ))
                }
            }
            Desire::Txt { content } => {
                if self.has_root_txt_content(content).await? {
                    Ok(())
                } else {
                    Err(format!(
                        "Bunny zone {} root TXT records do not contain desired content {:?}",
                        self.zone_id, content
                    ))
                }
            }
        }
    }

    async fn apply(&self, desire: &Desire) -> Result<(), String> {
        match desire {
            Desire::Subdomain { name } => self.upsert_subdomain_cname_to_root(name).await,
            Desire::Address { value } => self.upsert_root_ip(*value).await,
            Desire::Txt { content } => self.ensure_root_txt(content).await,
        }
    }
}
