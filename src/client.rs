use std::fs::File;
use std::io::{self, BufRead, Write};
use std::path::Path;
use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
struct SshKey {
    server: String,
    public_key: String,
}

fn read_known_hosts(file_path: &str) -> io::Result<Vec<SshKey>> {
    let path = Path::new(file_path);
    let file = File::open(&path)?;
    let reader = io::BufReader::new(file);

    let mut keys = Vec::new();
    for line in reader.lines() {
        if let Ok(line) = line {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                let server = parts[0].to_string();
                let public_key = parts[1..].join(" ");
                keys.push(SshKey { server, public_key });
            }
        }
    }
    Ok(keys)
}

fn write_known_hosts(file_path: &str, keys: &[SshKey]) -> io::Result<()> {
    let path = Path::new(file_path);
    let mut file = File::create(&path)?;

    for key in keys {
        writeln!(file, "{} {}", key.server, key.public_key)?;
    }

    Ok(())
}

async fn send_keys_to_server(host: &str, keys: Vec<SshKey>) -> Result<(), reqwest::Error> {
    let client = Client::new();
    let url = format!("{}/keys", host);
    let response = client.post(&url)
        .json(&keys)
        .send()
        .await?;

    if response.status().is_success() {
        println!("Keys successfully sent to server.");
    } else {
        eprintln!("Failed to send keys to server. Status: {}", response.status());
    }

    Ok(())
}

async fn get_keys_from_server(host: &str) -> Result<Vec<SshKey>, reqwest::Error> {
    let client = Client::new();
    let url = format!("{}/keys", host);
    let response = client.get(&url).send().await?;

    if response.status().is_success() {
        let keys: Vec<SshKey> = response.json().await?;
        Ok(keys)
    } else {
        eprintln!("Failed to get keys from server. Status: {}", response.status());
        Ok(vec![])
    }
}

pub async fn run_client(args: crate::Args) -> std::io::Result<()> {
    let keys = read_known_hosts(&args.known_hosts)
        .expect("Failed to read known hosts file");

    let host = args.host.expect("host is required in client mode");
    send_keys_to_server(&host, keys).await
        .expect("Failed to send keys to server");

    if args.in_place {
        let server_keys = get_keys_from_server(&host).await
            .expect("Failed to get keys from server");

        write_known_hosts(&args.known_hosts, &server_keys)
            .expect("Failed to write known hosts file");
    }

    Ok(())
}
