use serde::Deserialize;

#[derive(Debug, Deserialize, Default)]
pub struct Config {
    pub address: Option<AddressConfig>,
    #[serde(default)]
    pub subdomains: Vec<SubdomainConfig>,
    #[serde(default)]
    pub txt: Vec<TxtRecordConfig>,
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
