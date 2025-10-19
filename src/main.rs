use reqwest::Client;
use serde_derive::Deserialize;
use serde_derive::Serialize;
use std::{env, time::Duration};

#[derive(Deserialize)]
struct RecordList {
    records: Vec<DnsRecord>,
}

#[derive(Deserialize)]
struct DnsRecord {
    id: String,
    name: String,
    value: String,
    #[serde(rename = "type")]
    record_type: String,
    zone_id: String,
}

#[derive(Serialize)]
struct UpdateRecord<'a> {
    name: String,
    ttl: u32,
    #[serde(rename = "type")]
    record_type: String,
    value: &'a str,
    zone_id: String,
}

async fn get_current_ip() -> Result<(String, String), Box<dyn std::error::Error>> {
    let ipv4 = reqwest::get("https://api.ipify.org").await?.text().await?;
    let ipv6 = reqwest::get("https://api6.ipify.org").await?.text().await?;
    Ok((ipv4, ipv6))
}

async fn update_dns_record(
    client: &Client,
    token: &str,
    record: &DnsRecord,
    new_ip: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let url = format!("https://dns.hetzner.com/api/v1/records/{}", record.id);

    let payload = UpdateRecord {
        name: record.name.clone(),
        ttl: 60,
        record_type: record.record_type.clone(),
        value: new_ip,
        zone_id: record.zone_id.clone(),
    };

    client
        .put(&url)
        .header("Auth-API-Token".to_string(), token.to_string())
        .json(&payload)
        .send()
        .await?
        .error_for_status()?;

    println!("DNS record updated: {} -> {}", record.name, new_ip);
    Ok(())
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let token = env::var("HETZNER_DNS_TOKEN")?;
    let zone_id = env::var("ZONE_ID")?;
    let record_name = env::var("RECORD_NAME")?;

    let client = Client::new();

    loop {
        let ip = get_current_ip().await?;
        println!("Current ipv4: {}, ipv6: {}", ip.0, ip.1);

        let query = client
            .get("https://dns.hetzner.com/api/v1/records")
            .header("Auth-API-Token".to_string(), token.clone())
            .query(&[("zone_id", &zone_id)]);
        let resp = query.send().await?;
        let resp = resp.json::<RecordList>().await?;

        for record in resp.records.iter().filter(|r| r.name == record_name) {
            match record.record_type.as_str() {
                "A" => {
                    if record.value != ip.0 {
                        update_dns_record(&client, &token, record, &ip.0).await?;
                    } else {
                        println!("Identical IPv4, no update required");
                    }
                }

                "AAAA" => {
                    if record.value != ip.1 {
                        update_dns_record(&client, &token, record, &ip.1).await?;
                    } else {
                        println!("Identical IPv6, no update required");
                    }
                }
                _ => (),
            }
        }

        tokio::time::sleep(Duration::from_secs(300)).await;
    }
}

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("Error: {}", e);
    }
}
