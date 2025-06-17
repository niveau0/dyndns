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
    value: &'a str,
    ttl: u32,
}

async fn get_current_ip() -> Result<String, Box<dyn std::error::Error>> {
    let ip = reqwest::get("https://api.ipify.org").await?.text().await?;
    Ok(ip)
}

async fn update_dns_record(
    client: &Client,
    token: &str,
    record: &DnsRecord,
    new_ip: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let url = format!("https://dns.hetzner.com/api/v1/records/{}", record.id);

    let payload = UpdateRecord {
        value: new_ip,
        ttl: 60,
    };

    client
        .put(&url)
        .bearer_auth(token)
        .json(&payload)
        .send()
        .await?
        .error_for_status()?;

    println!("DNS record updated: {} -> {}", record.name, new_ip);
    Ok(())
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    dotenv::dotenv()?;

    let token = env::var("HETZNER_DNS_TOKEN")?;
    let zone_id = env::var("ZONE_ID")?;
    let record_name = env::var("RECORD_NAME")?;

    let client = Client::new();

    loop {
        let ip = get_current_ip().await?;

        let resp = client
            .get("https://dns.hetzner.com/api/v1/records")
            .bearer_auth(&token)
            .query(&[("zone_id", &zone_id)])
            .send()
            .await?
            .json::<RecordList>()
            .await?;

        if let Some(record) = resp
            .records
            .iter()
            .find(|r| r.name == record_name && r.record_type == "A")
        {
            if record.value != ip {
                update_dns_record(&client, &token, record, &ip).await?;
            } else {
                println!("Identical IP, no update required");
            }
        } else {
            println!("A-Record '{}' not found", record_name);
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
