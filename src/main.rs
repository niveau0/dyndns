use reqwest::Client;
use serde_derive::Deserialize;
use serde_derive::Serialize;
use std::env;

#[derive(Deserialize)]
struct RRSet {
    name: String,
    #[serde(rename = "type")]
    record_type: String,
    //ttl: u32,
    records: Vec<RRSetRecord>,
}

#[derive(Deserialize)]
struct RRSetRecord {
    value: String,
}

#[derive(Deserialize)]
struct RRSetList {
    rrsets: Vec<RRSet>,
}

#[derive(Serialize)]
struct CreateRRSet<'a> {
    name: String,
    #[serde(rename = "type")]
    record_type: String,
    ttl: u32,
    records: Vec<NewRecord<'a>>,
    labels: serde_json::Value,
}

#[derive(Serialize)]
struct NewRecord<'a> {
    value: &'a str,
    comment: &'a str,
}

async fn get_current_ip() -> Result<(String, String), Box<dyn std::error::Error>> {
    let ipv4 = reqwest::get("https://api.ipify.org").await?.text().await?;
    let ipv6 = reqwest::get("https://api6.ipify.org").await?.text().await?;
    Ok((ipv4, ipv6))
}

async fn create_rrset(
    client: &Client,
    token: &str,
    zone_id: &str,
    name: &str,
    record_type: &str,
    ip: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let payload = CreateRRSet {
        name: name.to_string(),
        record_type: record_type.to_string(),
        ttl: 60,
        records: vec![NewRecord {
            value: ip,
            comment: "",
        }],
        labels: serde_json::json!({}),
    };

    client
        .post(format!(
            "https://api.hetzner.cloud/v1/zones/{}/rrsets",
            zone_id
        ))
        .header("Authorization", format!("Bearer {}", token))
        .json(&payload)
        .send()
        .await?
        .error_for_status()?;

    Ok(())
}
async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let token =
        env::var("HETZNER_DNS_TOKEN").map_err(|_| "Missing HETZNER_DNS_TOKEN (api token)")?;
    let zone_id = env::var("ZONE_ID").map_err(|_| "Missing ZONE_ID (domain), see curl -H \"Authorization: Bearer $HETZNER_DNS_TOKEN\" https://api.hetzner.cloud/v1/zones")?;
    let record_names = env::var("RECORD_NAMES")
        .map_err(|_| "Missing RECORD_NAMES (your domains)")?
        .split(',')
        .map(|s| s.trim().to_string())
        .collect::<Vec<String>>();

    let client = Client::new();

    let ip = get_current_ip().await?;
    println!("Current ipv4: {}, ipv6: {}", ip.0, ip.1);

    let resp = client
        .get(format!(
            "https://api.hetzner.cloud/v1/zones/{}/rrsets",
            zone_id
        ))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await?
        .json::<RRSetList>()
        .await?;

    for record_name in &record_names {
        let existing_a = resp
            .rrsets
            .iter()
            .find(|r| r.name == *record_name && r.record_type == "A");
        let existing_aaaa = resp
            .rrsets
            .iter()
            .find(|r| r.name == *record_name && r.record_type == "AAAA");

        for (rrset_opt, current_ip, record_type) in
            [(existing_a, &ip.0, "A"), (existing_aaaa, &ip.1, "AAAA")]
        {
            match rrset_opt {
                Some(rrset) if rrset.records.iter().any(|r| &r.value == current_ip) => {
                    println!("Identical {}, no update required", record_type);
                }
                Some(rrset) => {
                    client
                        .delete(format!(
                            "https://api.hetzner.cloud/v1/zones/{}/rrsets/{}/{}",
                            zone_id, rrset.name, record_type
                        ))
                        .header("Authorization", format!("Bearer {}", token))
                        .send()
                        .await?
                        .error_for_status()?;

                    create_rrset(
                        &client,
                        &token,
                        &zone_id,
                        &record_name,
                        record_type,
                        current_ip,
                    )
                    .await?;
                    println!(
                        "DNS record updated: {} ({}) -> {}",
                        record_name, record_type, current_ip
                    );
                }
                None => {
                    create_rrset(
                        &client,
                        &token,
                        &zone_id,
                        &record_name,
                        record_type,
                        current_ip,
                    )
                    .await?;
                    println!(
                        "DNS record created: {} ({}) -> {}",
                        record_name, record_type, current_ip
                    );
                }
            }
        }
    }
    Ok(())
}

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("Error: {}", e);
    }
}
