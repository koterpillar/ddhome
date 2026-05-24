#![allow(dead_code)]

use std::future::Future;
use std::net::IpAddr;

use bunny_net_api::core::{CoreClient, DnsRecordType};

use crate::model::Desire;
use crate::provider::Provider;

pub struct BunnyProvider {
    api_key: String,
    zone_id: i64,
}

impl BunnyProvider {
    pub fn new(api_key: impl Into<String>, zone_id: i64) -> Self {
        Self {
            api_key: api_key.into(),
            zone_id,
        }
    }

    async fn has_root_ip(&self, expected: IpAddr) -> Result<bool, String> {
        let client = CoreClient::new(&self.api_key);
        let zone = client
            .get_dns_zone(self.zone_id)
            .await
            .map_err(|e| format!("failed to fetch DNS zone {}: {e}", self.zone_id))?;

        for record in zone.records {
            let is_root = record.name.is_empty() || record.name == "@";
            if !is_root {
                continue;
            }

            match expected {
                IpAddr::V4(expected_v4) => {
                    if record.record_type != Some(DnsRecordType::A) {
                        continue;
                    }

                    if let Ok(actual_v4) = record.value.parse::<std::net::Ipv4Addr>() {
                        if actual_v4 == expected_v4 {
                            return Ok(true);
                        }
                    }
                }
                IpAddr::V6(expected_v6) => {
                    if record.record_type != Some(DnsRecordType::AAAA) {
                        continue;
                    }

                    if let Ok(actual_v6) = record.value.parse::<std::net::Ipv6Addr>() {
                        if actual_v6 == expected_v6 {
                            return Ok(true);
                        }
                    }
                }
            }
        }

        Ok(false)
    }
}

impl Provider for BunnyProvider {
    fn evaluate<'a>(
        &'a self,
        desire: &'a Desire,
    ) -> impl Future<Output = Result<(), String>> + Send + 'a {
        async move {
            match desire {
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
                Desire::Txt { .. } => Err(
                    "BunnyProvider currently only supports Address desires for evaluate".to_owned(),
                ),
            }
        }
    }

    fn apply<'a>(
        &'a self,
        _desire: &'a Desire,
    ) -> impl Future<Output = Result<(), String>> + Send + 'a {
        async move { Err("BunnyProvider apply is not implemented yet".to_owned()) }
    }
}
