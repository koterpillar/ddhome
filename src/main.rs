use std::env;
use std::net::Ipv4Addr;

use bunny_net_api::core::{CoreClient, DnsRecordType};

mod bunny_auth;
mod config;
mod config_parser;
mod public_ip;

use bunny_auth::read_bunny_api_key;
use config_parser::parse_config_path;
use public_ip::{get_public_ipv4, get_public_ipv6};

async fn fetch_bunny_root_a_records(api_key: &str, zone_id: i64) -> Result<Vec<Ipv4Addr>, String> {
    let client = CoreClient::new(api_key);
    let zone = client
        .get_dns_zone(zone_id)
        .await
        .map_err(|e| format!("failed to fetch DNS zone {zone_id}: {e}"))?;

    let mut ips = Vec::new();
    for record in zone.records {
        if record.record_type != Some(DnsRecordType::A) {
            continue;
        }

        if !(record.name.is_empty() || record.name == "@") {
            continue;
        }

        let ip: Ipv4Addr = record.value.parse().map_err(|e| {
            format!(
                "invalid IPv4 value '{}' in Bunny DNS record {}: {e}",
                record.value, record.id
            )
        })?;
        ips.push(ip);
    }

    Ok(ips)
}

async fn main_res() -> Result<(), String> {
    let bunny_api_key = read_bunny_api_key()?;

    let config_path = env::args()
        .nth(1)
        .unwrap_or_else(|| "/etc/ddhome".to_owned());

    let cfg = parse_config_path(&config_path)?;

    if let Some(bunny) = &cfg.bunny {
        match fetch_bunny_root_a_records(&bunny_api_key, bunny.zone_id).await {
            Ok(ips) if ips.is_empty() => {
                println!("no Bunny root A records found in zone {}", bunny.zone_id)
            }
            Ok(ips) => {
                println!("Bunny root A records in zone {}: {:?}", bunny.zone_id, ips)
            }
            Err(e) => {
                eprintln!("failed to query Bunny A records: {e}");
            }
        }
    }

    println!("loaded config");
    if let Some(address) = &cfg.address {
        println!(
            "address records enabled: a={}, aaaa={}",
            address.a, address.aaaa
        );

        if address.a {
            match get_public_ipv4().await {
                Ok(ip) => println!("detected public IPv4: {ip}"),
                Err(e) => eprintln!("failed to detect public IPv4: {e}"),
            }
        }

        if address.aaaa {
            match get_public_ipv6().await {
                Ok(ip) => println!("detected public IPv6: {ip}"),
                Err(e) => eprintln!("failed to detect public IPv6: {e}"),
            }
        }
    }
    println!("subdomain entries: {}", cfg.subdomains.len());
    if let Some(first) = cfg.subdomains.first() {
        println!("first subdomain: {}", first.name);
    }
    println!("txt entries: {}", cfg.txt.len());
    if let Some(first) = cfg.txt.first() {
        println!("first txt record length: {}", first.content.len());
    }
    Ok(())
}

#[tokio::main]
async fn main() {
    match main_res().await {
        Ok(v) => v,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };
}
