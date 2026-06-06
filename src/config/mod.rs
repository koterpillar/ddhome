pub mod parser;

use semigroup::Semigroup;
use serde::Deserialize;

use crate::model::CaaRecord;

pub use parser::parse_config_path;

#[derive(Debug, Deserialize, Semigroup)]
#[semigroup(monoid)]
pub struct Config {
    #[semigroup(with = "semigroup::op::Overwrite")]
    pub bunny: Option<BunnyConfig>,
    #[semigroup(with = "semigroup::op::Overwrite")]
    pub address: Option<AddressConfig>,
    #[serde(default)]
    #[semigroup(with = "semigroup::op::Concat")]
    pub subdomain: Vec<SubdomainConfig>,
    #[serde(default)]
    #[semigroup(with = "semigroup::op::Concat")]
    pub txt: Vec<TxtRecordConfig>,
    #[serde(default)]
    #[semigroup(with = "semigroup::op::Concat")]
    pub caa: Vec<CaaRecordConfig>,
}

#[derive(Debug, Deserialize)]
pub struct BunnyConfig {
    pub zone_id: i64,
}

#[derive(Debug, Deserialize)]
pub struct AddressConfig {
    #[serde(default)]
    pub a: bool,
    #[serde(default)]
    pub aaaa: bool,
}

#[derive(Debug, Deserialize)]
pub struct SubdomainConfig {
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct TxtRecordConfig {
    pub content: String,
}

#[derive(Debug, Deserialize)]
pub struct CaaRecordConfig {
    pub ca: String,
    #[serde(default)]
    pub wildcards: bool,
    #[serde(default)]
    pub account_uri: Option<String>,
    #[serde(default)]
    pub validation_methods: Option<Vec<String>>,
}

impl CaaRecordConfig {
    pub fn to_caa_record(&self) -> CaaRecord {
        CaaRecord::new(
            &self.ca,
            self.wildcards,
            self.account_uri.as_deref(),
            self.validation_methods.as_deref(),
        )
    }
}
