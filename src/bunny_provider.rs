#![allow(dead_code)]

use std::net::IpAddr;
use std::sync::Mutex;

use bunny_net_api::core::{CoreClient, DnsRecord, DnsRecordType, DnsZone};

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

    async fn apply(&self, _desire: &Desire) -> Result<(), String> {
        Err("BunnyProvider apply is not implemented yet".to_owned())
    }
}
