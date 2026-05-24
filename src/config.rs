use serde::Deserialize;

#[derive(Debug, Deserialize, Default)]
pub struct Config {
    pub bunny: Option<BunnyConfig>,
    pub address: Option<AddressConfig>,
    #[serde(default)]
    pub subdomain: Vec<SubdomainConfig>,
    #[serde(default)]
    pub txt: Vec<TxtRecordConfig>,
    #[serde(default)]
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
}
