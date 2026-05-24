use std::env;
use std::net::IpAddr;

mod bunny_auth;
mod bunny_provider;
mod config;
mod config_parser;
mod model;
mod provider;
mod public_ip;

use bunny_auth::read_bunny_api_key;
use bunny_provider::BunnyProvider;
use config_parser::parse_config_path;
use model::{Desire, Desires};
use provider::Provider;
use public_ip::{get_public_ipv4, get_public_ipv6};

async fn desires_from_config(cfg: &config::Config) -> Result<Desires, String> {
    let mut desires = Vec::new();

    for subdomain in &cfg.subdomain {
        desires.push(Desire::Subdomain {
            name: subdomain.name.clone(),
        });
    }

    for txt in &cfg.txt {
        desires.push(Desire::Txt {
            content: txt.content.clone(),
        });
    }

    if let Some(address) = &cfg.address {
        if address.a {
            let ip = get_public_ipv4().await?;
            desires.push(Desire::Address {
                value: IpAddr::V4(ip),
            });
        }

        if address.aaaa {
            let ip = get_public_ipv6().await?;
            desires.push(Desire::Address {
                value: IpAddr::V6(ip),
            });
        }
    }

    Ok(desires)
}

fn make_provider(cfg: &config::Config) -> Result<impl Provider + Sync, String> {
    let bunny = cfg
        .bunny
        .as_ref()
        .ok_or_else(|| "no provider configured; missing [bunny] section".to_owned())?;

    let bunny_api_key = read_bunny_api_key()?;
    Ok(BunnyProvider::new(bunny_api_key, bunny.zone_id))
}

async fn evaluate_desires(
    provider: &(impl Provider + Sync),
    desires: &Desires,
) -> Result<(), String> {
    for (desire, evaluation) in provider.evaluate_desires(desires).await {
        match evaluation {
            Ok(()) => println!("satisfied desire: {:?}", desire),
            Err(explanation) => {
                println!("mismatch for desire {:?}: {explanation}", desire)
            }
        }
    }

    Ok(())
}

async fn main_res() -> Result<(), String> {
    let config_path = env::args()
        .nth(1)
        .unwrap_or_else(|| "/etc/ddhome".to_owned());

    let cfg = parse_config_path(&config_path)?;
    let desires = desires_from_config(&cfg).await?;
    let provider = make_provider(&cfg)?;

    evaluate_desires(&provider, &desires).await?;

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
