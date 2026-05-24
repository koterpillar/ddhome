use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::time::Duration;

const IPV4_ENDPOINTS: [&str; 2] = ["https://api.ipify.org", "https://ipv4.icanhazip.com"];
const IPV6_ENDPOINTS: [&str; 2] = ["https://api6.ipify.org", "https://ipv6.icanhazip.com"];

#[derive(Debug, Clone, Copy)]
pub enum IpVersion {
    V4,
    V6,
}

pub async fn get_public_ip(version: IpVersion) -> Result<IpAddr, String> {
    let endpoints = match version {
        IpVersion::V4 => &IPV4_ENDPOINTS,
        IpVersion::V6 => &IPV6_ENDPOINTS,
    };

    let mut errors = Vec::new();
    for endpoint in endpoints {
        match fetch_ip(endpoint, version).await {
            Ok(ip) => return Ok(ip),
            Err(err) => errors.push(format!("{endpoint}: {err}")),
        }
    }

    Err(format!(
        "failed to resolve public {:?} address from all endpoints: {}",
        version,
        errors.join("; ")
    ))
}

pub async fn get_public_ipv4() -> Result<Ipv4Addr, String> {
    match get_public_ip(IpVersion::V4).await? {
        IpAddr::V4(ip) => Ok(ip),
        IpAddr::V6(_) => Err("endpoint returned an IPv6 address while IPv4 was requested".to_owned()),
    }
}

pub async fn get_public_ipv6() -> Result<Ipv6Addr, String> {
    match get_public_ip(IpVersion::V6).await? {
        IpAddr::V6(ip) => Ok(ip),
        IpAddr::V4(_) => Err("endpoint returned an IPv4 address while IPv6 was requested".to_owned()),
    }
}

async fn fetch_ip(endpoint: &str, version: IpVersion) -> Result<IpAddr, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(|e| format!("failed to create HTTP client: {e}"))?;

    let response = client
        .get(endpoint)
        .send()
        .await
        .map_err(|e| format!("request failed: {e}"))?
        .error_for_status()
        .map_err(|e| format!("request failed: {e}"))?;

    let raw = response
        .text()
        .await
        .map_err(|e| format!("failed to read response body: {e}"))?;
    let ip: IpAddr = raw
        .trim()
        .parse()
        .map_err(|e| format!("failed to parse IP address from response: {e}"))?;

    match (version, ip) {
        (IpVersion::V4, IpAddr::V4(_)) => Ok(ip),
        (IpVersion::V6, IpAddr::V6(_)) => Ok(ip),
        (IpVersion::V4, IpAddr::V6(_)) => Err("received IPv6 for IPv4 query".to_owned()),
        (IpVersion::V6, IpAddr::V4(_)) => Err("received IPv4 for IPv6 query".to_owned()),
    }
}
