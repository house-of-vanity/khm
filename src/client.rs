use base64::{engine::general_purpose, Engine as _};
use log::{error, info};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{self, BufRead, Write};
use std::path::Path;

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
        match line {
            Ok(line) => {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    let server = parts[0].to_string();
                    let public_key = parts[1..].join(" ");
                    keys.push(SshKey { server, public_key });
                }
            }
            Err(e) => {
                error!("Error reading line from known_hosts file: {}", e);
            }
        }
    }
    info!("Read {} keys from known_hosts file", keys.len());
    Ok(keys)
}

fn write_known_hosts(file_path: &str, keys: &[SshKey]) -> io::Result<()> {
    let path = Path::new(file_path);
    let mut file = File::create(&path)?;

    for key in keys {
        writeln!(file, "{} {}", key.server, key.public_key)?;
    }
    info!("Wrote {} keys to known_hosts file", keys.len());

    Ok(())
}

// Get local hostname for request headers
fn get_hostname() -> String {
    match hostname::get() {
        Ok(name) => name.to_string_lossy().to_string(),
        Err(_) => "unknown-host".to_string(),
    }
}

async fn send_keys_to_server(
    host: &str,
    keys: Vec<SshKey>,
    auth_string: &str,
) -> Result<(), reqwest::Error> {
    let client = Client::new();
    let url = format!("{}/keys", host);
    info!("URL: {} ", url);

    let mut headers = HeaderMap::new();

    // Add hostname header
    let hostname = get_hostname();
    headers.insert(
        "X-Client-Hostname",
        HeaderValue::from_str(&hostname).unwrap_or_else(|_| {
            error!("Failed to create hostname header value");
            HeaderValue::from_static("unknown-host")
        }),
    );
    info!("Adding hostname header: {}", hostname);

    if !auth_string.is_empty() {
        let parts: Vec<&str> = auth_string.splitn(2, ':').collect();
        if parts.len() == 2 {
            let username = parts[0];
            let password = parts[1];

            let auth_header_value = format!("{}:{}", username, password);
            let encoded_auth = general_purpose::STANDARD.encode(auth_header_value);
            let auth_header = format!("Basic {}", encoded_auth);

            headers.insert(AUTHORIZATION, HeaderValue::from_str(&auth_header).unwrap());
        } else {
            error!("Invalid auth string format. Expected 'username:password'");
        }
    }

    let response = client
        .post(&url)
        .headers(headers)
        .json(&keys)
        .send()
        .await?;

    if response.status().is_success() {
        info!("Keys successfully sent to server.");
    } else {
        error!(
            "Failed to send keys to server. Status: {}",
            response.status()
        );
    }

    Ok(())
}

async fn get_keys_from_server(
    host: &str,
    auth_string: &str,
) -> Result<Vec<SshKey>, reqwest::Error> {
    let client = Client::new();
    let url = format!("{}/keys", host);

    let mut headers = HeaderMap::new();

    // Add hostname header
    let hostname = get_hostname();
    headers.insert(
        "X-Client-Hostname",
        HeaderValue::from_str(&hostname).unwrap_or_else(|_| {
            error!("Failed to create hostname header value");
            HeaderValue::from_static("unknown-host")
        }),
    );
    info!("Adding hostname header: {}", hostname);

    if !auth_string.is_empty() {
        let parts: Vec<&str> = auth_string.splitn(2, ':').collect();
        if parts.len() == 2 {
            let username = parts[0];
            let password = parts[1];

            let auth_header_value = format!("{}:{}", username, password);
            let encoded_auth = general_purpose::STANDARD.encode(auth_header_value);
            let auth_header = format!("Basic {}", encoded_auth);

            headers.insert(AUTHORIZATION, HeaderValue::from_str(&auth_header).unwrap());
        } else {
            error!("Invalid auth string format. Expected 'username:password'");
        }
    }

    let response = client.get(&url).headers(headers).send().await?;

    let response = response.error_for_status()?;

    let keys: Vec<SshKey> = response.json().await?;
    info!("Received {} keys from server", keys.len());
    Ok(keys)
}

pub async fn run_client(args: crate::Args) -> std::io::Result<()> {
    info!("Client mode: Reading known_hosts file");
    let keys = read_known_hosts(&args.known_hosts).expect("Failed to read known hosts file");

    let host = args.host.expect("host is required in client mode");
    info!("Client mode: Sending keys to server at {}", host);
    send_keys_to_server(&host, keys, &args.basic_auth)
        .await
        .expect("Failed to send keys to server");

    if args.in_place {
        info!("Client mode: In-place update is enabled. Fetching keys from server.");
        let server_keys = get_keys_from_server(&host, &args.basic_auth)
            .await
            .expect("Failed to get keys from server");

        info!("Client mode: Writing updated known_hosts file");
        write_known_hosts(&args.known_hosts, &server_keys)
            .expect("Failed to write known hosts file");
    }

    info!("Client mode: Finished operations");
    Ok(())
}
