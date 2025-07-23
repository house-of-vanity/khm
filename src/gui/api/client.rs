use crate::gui::common::{perform_sync, KhmSettings};
use log::info;
use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SshKey {
    pub server: String,
    pub public_key: String,
    #[serde(default)]
    pub deprecated: bool,
}

/// Test connection to KHM server
pub async fn test_connection(
    host: String,
    flow: String,
    basic_auth: String,
) -> Result<String, String> {
    if host.is_empty() || flow.is_empty() {
        return Err("Host and flow must be specified".to_string());
    }

    let url = format!("{}/{}/keys", host.trim_end_matches('/'), flow);
    info!("Testing connection to: {}", url);

    let client = create_http_client()?;
    let mut request = client.get(&url);

    request = add_auth_if_needed(request, &basic_auth)?;

    let response = request
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    check_response_status(&response)?;

    let body = response
        .text()
        .await
        .map_err(|e| format!("Failed to read response: {}", e))?;

    check_html_response(&body)?;

    let keys: Vec<SshKey> =
        serde_json::from_str(&body).map_err(|e| format!("Failed to parse response: {}", e))?;

    let message = format!("Found {} SSH keys from flow '{}'", keys.len(), flow);
    info!("Connection test successful: {}", message);
    Ok(message)
}

/// Fetch all SSH keys including deprecated ones
pub async fn fetch_keys(
    host: String,
    flow: String,
    basic_auth: String,
) -> Result<Vec<SshKey>, String> {
    if host.is_empty() || flow.is_empty() {
        return Err("Host and flow must be specified".to_string());
    }

    let url = format!(
        "{}/{}/keys?include_deprecated=true",
        host.trim_end_matches('/'),
        flow
    );
    info!("Fetching keys from: {}", url);

    let client = create_http_client()?;
    let mut request = client.get(&url);

    request = add_auth_if_needed(request, &basic_auth)?;

    let response = request
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    check_response_status(&response)?;

    let body = response
        .text()
        .await
        .map_err(|e| format!("Failed to read response: {}", e))?;

    check_html_response(&body)?;

    let keys: Vec<SshKey> =
        serde_json::from_str(&body).map_err(|e| format!("Failed to parse response: {}", e))?;

    info!("Fetched {} SSH keys", keys.len());
    Ok(keys)
}

/// Deprecate a key for a specific server
pub async fn deprecate_key(
    host: String,
    flow: String,
    basic_auth: String,
    server: String,
) -> Result<String, String> {
    let url = format!(
        "{}/{}/keys/{}",
        host.trim_end_matches('/'),
        flow,
        urlencoding::encode(&server)
    );
    info!("Deprecating key for server '{}' at: {}", server, url);

    let client = create_http_client()?;
    let mut request = client.delete(&url);

    request = add_auth_if_needed(request, &basic_auth)?;

    let response = request
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    check_response_status(&response)?;

    let body = response
        .text()
        .await
        .map_err(|e| format!("Failed to read response: {}", e))?;

    parse_api_response(
        &body,
        &format!("Successfully deprecated key for server '{}'", server),
    )
}

/// Restore a key for a specific server
pub async fn restore_key(
    host: String,
    flow: String,
    basic_auth: String,
    server: String,
) -> Result<String, String> {
    let url = format!(
        "{}/{}/keys/{}/restore",
        host.trim_end_matches('/'),
        flow,
        urlencoding::encode(&server)
    );
    info!("Restoring key for server '{}' at: {}", server, url);

    let client = create_http_client()?;
    let mut request = client.post(&url);

    request = add_auth_if_needed(request, &basic_auth)?;

    let response = request
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    check_response_status(&response)?;

    let body = response
        .text()
        .await
        .map_err(|e| format!("Failed to read response: {}", e))?;

    parse_api_response(
        &body,
        &format!("Successfully restored key for server '{}'", server),
    )
}

/// Delete a key permanently for a specific server
pub async fn delete_key(
    host: String,
    flow: String,
    basic_auth: String,
    server: String,
) -> Result<String, String> {
    let url = format!(
        "{}/{}/keys/{}/delete",
        host.trim_end_matches('/'),
        flow,
        urlencoding::encode(&server)
    );
    info!(
        "Permanently deleting key for server '{}' at: {}",
        server, url
    );

    let client = create_http_client()?;
    let mut request = client.delete(&url);

    request = add_auth_if_needed(request, &basic_auth)?;

    let response = request
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    check_response_status(&response)?;

    let body = response
        .text()
        .await
        .map_err(|e| format!("Failed to read response: {}", e))?;

    parse_api_response(
        &body,
        &format!("Successfully deleted key for server '{}'", server),
    )
}

/// Bulk deprecate multiple servers
pub async fn bulk_deprecate_servers(
    host: String,
    flow: String,
    basic_auth: String,
    servers: Vec<String>,
) -> Result<String, String> {
    let url = format!("{}/{}/bulk-deprecate", host.trim_end_matches('/'), flow);
    info!("Bulk deprecating {} servers at: {}", servers.len(), url);

    let client = create_http_client()?;
    let mut request = client.post(&url).json(&serde_json::json!({
        "servers": servers
    }));

    request = add_auth_if_needed(request, &basic_auth)?;

    let response = request
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    check_response_status(&response)?;

    let body = response
        .text()
        .await
        .map_err(|e| format!("Failed to read response: {}", e))?;

    parse_api_response(&body, "Successfully deprecated servers")
}

/// Bulk restore multiple servers
pub async fn bulk_restore_servers(
    host: String,
    flow: String,
    basic_auth: String,
    servers: Vec<String>,
) -> Result<String, String> {
    let url = format!("{}/{}/bulk-restore", host.trim_end_matches('/'), flow);
    info!("Bulk restoring {} servers at: {}", servers.len(), url);

    let client = create_http_client()?;
    let mut request = client.post(&url).json(&serde_json::json!({
        "servers": servers
    }));

    request = add_auth_if_needed(request, &basic_auth)?;

    let response = request
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    check_response_status(&response)?;

    let body = response
        .text()
        .await
        .map_err(|e| format!("Failed to read response: {}", e))?;

    parse_api_response(&body, "Successfully restored servers")
}

/// Perform manual sync operation
pub async fn perform_manual_sync(settings: KhmSettings) -> Result<String, String> {
    match perform_sync(&settings).await {
        Ok(keys_count) => Ok(format!(
            "Sync completed successfully with {} keys",
            keys_count
        )),
        Err(e) => Err(e.to_string()),
    }
}

// Helper functions

fn create_http_client() -> Result<Client, String> {
    Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))
}

fn add_auth_if_needed(
    request: reqwest::RequestBuilder,
    basic_auth: &str,
) -> Result<reqwest::RequestBuilder, String> {
    if basic_auth.is_empty() {
        return Ok(request);
    }

    let auth_parts: Vec<&str> = basic_auth.splitn(2, ':').collect();
    if auth_parts.len() == 2 {
        Ok(request.basic_auth(auth_parts[0], Some(auth_parts[1])))
    } else {
        Err("Basic auth format should be 'username:password'".to_string())
    }
}

fn check_response_status(response: &reqwest::Response) -> Result<(), String> {
    let status = response.status().as_u16();

    if status == 401 {
        return Err(
            "Authentication required. Please provide valid basic auth credentials.".to_string(),
        );
    }

    if status >= 300 && status < 400 {
        return Err("Server redirects to login page. Authentication may be required.".to_string());
    }

    if !response.status().is_success() {
        return Err(format!(
            "Server returned error: {} {}",
            status,
            response.status().canonical_reason().unwrap_or("Unknown")
        ));
    }

    Ok(())
}

fn check_html_response(body: &str) -> Result<(), String> {
    if body.trim_start().starts_with("<!DOCTYPE") || body.trim_start().starts_with("<html") {
        return Err("Server returned HTML page instead of JSON. This usually means authentication is required or the endpoint is incorrect.".to_string());
    }
    Ok(())
}

fn parse_api_response(body: &str, default_message: &str) -> Result<String, String> {
    if let Ok(json_response) = serde_json::from_str::<serde_json::Value>(body) {
        if let Some(message) = json_response.get("message").and_then(|v| v.as_str()) {
            Ok(message.to_string())
        } else {
            Ok(default_message.to_string())
        }
    } else {
        Ok(default_message.to_string())
    }
}
