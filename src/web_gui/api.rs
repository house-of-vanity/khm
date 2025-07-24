use super::state::{SshKey, DnsResult, AdminSettings};
use log::info;
use reqwest::Client;
use std::time::Duration;

/// Create HTTP client for API requests
fn create_http_client() -> Result<Client, String> {
    Client::builder()
        .timeout(Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))
}

/// Add basic auth to request if provided
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

/// Check response status for errors
fn check_response_status(response: &reqwest::Response) -> Result<(), String> {
    let status = response.status().as_u16();
    
    if status == 401 {
        return Err("Authentication required. Please provide valid basic auth credentials.".to_string());
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

/// Check if response is HTML instead of JSON
fn check_html_response(body: &str) -> Result<(), String> {
    if body.trim_start().starts_with("<!DOCTYPE") || body.trim_start().starts_with("<html") {
        return Err("Server returned HTML page instead of JSON. This usually means authentication is required or the endpoint is incorrect.".to_string());
    }
    Ok(())
}

/// Get application version from API
pub async fn get_version(settings: &AdminSettings) -> Result<String, String> {
    if settings.server_url.is_empty() {
        return Err("Server URL must be specified".to_string());
    }
    
    let url = format!("{}/api/version", settings.server_url.trim_end_matches('/'));
    info!("Getting version from: {}", url);
    
    let client = create_http_client()?;
    let mut request = client.get(&url);
    
    request = add_auth_if_needed(request, &settings.basic_auth)?;
    
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
    
    let version_response: serde_json::Value = serde_json::from_str(&body)
        .map_err(|e| format!("Failed to parse version: {}", e))?;
        
    let version = version_response
        .get("version")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();
        
    info!("KHM server version: {}", version);
    Ok(version)
}

/// Test connection to KHM server using existing API endpoint
pub async fn test_connection(settings: &AdminSettings) -> Result<String, String> {
    if settings.server_url.is_empty() || settings.selected_flow.is_empty() {
        return Err("Server URL and flow must be specified".to_string());
    }
    
    let url = format!(
        "{}/{}/keys",
        settings.server_url.trim_end_matches('/'),
        settings.selected_flow
    );
    info!("Testing connection to: {}", url);
    
    let client = create_http_client()?;
    let mut request = client.get(&url);
    
    request = add_auth_if_needed(request, &settings.basic_auth)?;
    
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
    
    let keys: Vec<SshKey> = serde_json::from_str(&body)
        .map_err(|e| format!("Failed to parse response: {}", e))?;
        
    let message = format!("Connection successful! Found {} SSH keys from flow '{}'", keys.len(), settings.selected_flow);
    info!("{}", message);
    Ok(message)
}

/// Load available flows from server
pub async fn load_flows(settings: &AdminSettings) -> Result<Vec<String>, String> {
    if settings.server_url.is_empty() {
        return Err("Server URL must be specified".to_string());
    }
    
    let url = format!("{}/api/flows", settings.server_url.trim_end_matches('/'));
    info!("Loading flows from: {}", url);
    
    let client = create_http_client()?;
    let mut request = client.get(&url);
    
    request = add_auth_if_needed(request, &settings.basic_auth)?;
    
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
    
    let flows: Vec<String> = serde_json::from_str(&body)
        .map_err(|e| format!("Failed to parse flows: {}", e))?;
        
    info!("Loaded {} flows", flows.len());
    Ok(flows)
}

/// Fetch all SSH keys including deprecated ones using existing API endpoint
pub async fn fetch_keys(settings: &AdminSettings) -> Result<Vec<SshKey>, String> {
    if settings.server_url.is_empty() || settings.selected_flow.is_empty() {
        return Err("Server URL and flow must be specified".to_string());
    }
    
    let url = format!(
        "{}/{}/keys",
        settings.server_url.trim_end_matches('/'),
        settings.selected_flow
    );
    info!("Fetching keys from: {}", url);
    
    let client = create_http_client()?;
    let mut request = client.get(&url);
    
    request = add_auth_if_needed(request, &settings.basic_auth)?;
    
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
    
    let keys: Vec<SshKey> = serde_json::from_str(&body)
        .map_err(|e| format!("Failed to parse keys: {}", e))?;
        
    info!("Fetched {} SSH keys", keys.len());
    Ok(keys)
}

/// Deprecate a key for a specific server
pub async fn deprecate_key(
    settings: &AdminSettings,
    server: &str,
) -> Result<String, String> {
    let url = format!(
        "{}/{}/keys/{}",
        settings.server_url.trim_end_matches('/'),
        settings.selected_flow,
        urlencoding::encode(server)
    );
    info!("Deprecating key for server '{}' at: {}", server, url);
    
    let client = create_http_client()?;
    let mut request = client.delete(&url);
    
    request = add_auth_if_needed(request, &settings.basic_auth)?;
    
    let response = request
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;
        
    check_response_status(&response)?;
    
    Ok(format!("Successfully deprecated key for server '{}'", server))
}

/// Restore a key for a specific server
pub async fn restore_key(
    settings: &AdminSettings,
    server: &str,
) -> Result<String, String> {
    let url = format!(
        "{}/{}/keys/{}/restore",
        settings.server_url.trim_end_matches('/'),
        settings.selected_flow,
        urlencoding::encode(server)
    );
    info!("Restoring key for server '{}' at: {}", server, url);
    
    let client = create_http_client()?;
    let mut request = client.post(&url);
    
    request = add_auth_if_needed(request, &settings.basic_auth)?;
    
    let response = request
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;
        
    check_response_status(&response)?;
    
    Ok(format!("Successfully restored key for server '{}'", server))
}

/// Delete a key permanently for a specific server
pub async fn delete_key(
    settings: &AdminSettings,
    server: &str,
) -> Result<String, String> {
    let url = format!(
        "{}/{}/keys/{}/delete",
        settings.server_url.trim_end_matches('/'),
        settings.selected_flow,
        urlencoding::encode(server)
    );
    info!("Permanently deleting key for server '{}' at: {}", server, url);
    
    let client = create_http_client()?;
    let mut request = client.delete(&url);
    
    request = add_auth_if_needed(request, &settings.basic_auth)?;
    
    let response = request
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;
        
    check_response_status(&response)?;
    
    Ok(format!("Successfully deleted key for server '{}'", server))
}

/// Bulk deprecate multiple servers
pub async fn bulk_deprecate_servers(
    settings: &AdminSettings,
    servers: Vec<String>,
) -> Result<String, String> {
    let url = format!(
        "{}/{}/bulk-deprecate",
        settings.server_url.trim_end_matches('/'),
        settings.selected_flow
    );
    info!("Bulk deprecating {} servers at: {}", servers.len(), url);
    
    let client = create_http_client()?;
    let mut request = client.post(&url).json(&serde_json::json!({
        "servers": servers
    }));
    
    request = add_auth_if_needed(request, &settings.basic_auth)?;
    
    let response = request
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;
        
    check_response_status(&response)?;
    
    Ok("Successfully deprecated selected servers".to_string())
}

/// Bulk restore multiple servers
pub async fn bulk_restore_servers(
    settings: &AdminSettings,
    servers: Vec<String>,
) -> Result<String, String> {
    let url = format!(
        "{}/{}/bulk-restore",
        settings.server_url.trim_end_matches('/'),
        settings.selected_flow
    );
    info!("Bulk restoring {} servers at: {}", servers.len(), url);
    
    let client = create_http_client()?;
    let mut request = client.post(&url).json(&serde_json::json!({
        "servers": servers
    }));
    
    request = add_auth_if_needed(request, &settings.basic_auth)?;
    
    let response = request
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;
        
    check_response_status(&response)?;
    
    Ok("Successfully restored selected servers".to_string())
}

/// Scan DNS resolution for servers using existing API endpoint
pub async fn scan_dns_resolution(
    settings: &AdminSettings,
) -> Result<Vec<DnsResult>, String> {
    let url = format!(
        "{}/{}/scan-dns",
        settings.server_url.trim_end_matches('/'),
        settings.selected_flow
    );
    info!("Scanning DNS resolution at: {}", url);
    
    let client = create_http_client()?;
    let mut request = client.post(&url);
    
    request = add_auth_if_needed(request, &settings.basic_auth)?;
    
    let response = request
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;
        
    check_response_status(&response)?;
    
    let body = response
        .text()
        .await
        .map_err(|e| format!("Failed to read response: {}", e))?;
        
    // Parse the response format from existing API: {"results": [...], "total": N, "unresolved": N}
    let api_response: serde_json::Value = serde_json::from_str(&body)
        .map_err(|e| format!("Failed to parse DNS response: {}", e))?;
        
    let results = api_response
        .get("results")
        .and_then(|r| serde_json::from_value(r.clone()).ok())
        .unwrap_or_else(Vec::new);
        
    info!("DNS scan completed for {} servers", results.len());
    Ok(results)
}