use std::env;

mod config;
mod config_parser;
mod public_ip;

use config_parser::parse_config_path;
use public_ip::{get_public_ipv4, get_public_ipv6};

fn main() {
    let config_path = env::args()
        .nth(1)
        .unwrap_or_else(|| "/etc/ddhome".to_owned());

    let cfg = match parse_config_path(&config_path) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };

    println!("loaded config");
    if let Some(address) = &cfg.address {
        println!(
            "address records enabled: a={}, aaaa={}",
            address.a, address.aaaa
        );

        if address.a {
            match get_public_ipv4() {
                Ok(ip) => println!("detected public IPv4: {ip}"),
                Err(e) => eprintln!("failed to detect public IPv4: {e}"),
            }
        }

        if address.aaaa {
            match get_public_ipv6() {
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
}
