#[cfg(all(target_arch = "wasm32", feature = "web-gui"))]
use super::state::{SshKey, DnsResult, AdminSettings};
#[cfg(all(target_arch = "wasm32", feature = "web-gui"))]
use wasm_bindgen::prelude::*;
#[cfg(all(target_arch = "wasm32", feature = "web-gui"))]
use wasm_bindgen_futures::JsFuture;
#[cfg(all(target_arch = "wasm32", feature = "web-gui"))]
use web_sys::{Request, RequestInit, RequestMode, Response};

/// Simplified API for WASM - uses browser fetch API
#[cfg(all(target_arch = "wasm32", feature = "web-gui"))]
pub async fn test_connection(settings: &AdminSettings) -> Result<String, String> {
    let url = format!("{}/{}/keys", settings.server_url.trim_end_matches('/'), settings.selected_flow);
    
    let response = fetch_json(&url).await?;
    let keys: Result<Vec<SshKey>, _> = serde_json::from_str(&response);
    
    match keys {
        Ok(keys) => Ok(format!("Connection successful! Found {} SSH keys from flow '{}'", keys.len(), settings.selected_flow)),
        Err(e) => Err(format!("Failed to parse response: {}", e)),
    }
}

#[cfg(all(target_arch = "wasm32", feature = "web-gui"))]
pub async fn load_flows(settings: &AdminSettings) -> Result<Vec<String>, String> {
    let url = format!("{}/api/flows", settings.server_url.trim_end_matches('/'));
    
    let response = fetch_json(&url).await?;
    let flows: Result<Vec<String>, _> = serde_json::from_str(&response);
    
    flows.map_err(|e| format!("Failed to parse flows: {}", e))
}

#[cfg(all(target_arch = "wasm32", feature = "web-gui"))]
pub async fn fetch_keys(settings: &AdminSettings) -> Result<Vec<SshKey>, String> {
    let url = format!("{}/{}/keys", settings.server_url.trim_end_matches('/'), settings.selected_flow);
    
    let response = fetch_json(&url).await?;
    let keys: Result<Vec<SshKey>, _> = serde_json::from_str(&response);
    
    keys.map_err(|e| format!("Failed to parse keys: {}", e))
}

#[cfg(all(target_arch = "wasm32", feature = "web-gui"))]
pub async fn get_version(settings: &AdminSettings) -> Result<String, String> {
    let url = format!("{}/api/version", settings.server_url.trim_end_matches('/'));
    
    let response = fetch_json(&url).await?;
    let version_response: Result<serde_json::Value, _> = serde_json::from_str(&response);
    
    match version_response {
        Ok(data) => {
            let version = data.get("version")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            Ok(version)
        }
        Err(e) => Err(format!("Failed to parse version: {}", e)),
    }
}

#[cfg(all(target_arch = "wasm32", feature = "web-gui"))]
pub async fn deprecate_key(_settings: &AdminSettings, server: &str) -> Result<String, String> {
    Ok(format!("WASM: Would deprecate key for {}", server))
}

#[cfg(all(target_arch = "wasm32", feature = "web-gui"))]
pub async fn restore_key(_settings: &AdminSettings, server: &str) -> Result<String, String> {
    Ok(format!("WASM: Would restore key for {}", server))
}

#[cfg(all(target_arch = "wasm32", feature = "web-gui"))]
pub async fn delete_key(_settings: &AdminSettings, server: &str) -> Result<String, String> {
    Ok(format!("WASM: Would delete key for {}", server))
}

#[cfg(all(target_arch = "wasm32", feature = "web-gui"))]
pub async fn bulk_deprecate_servers(_settings: &AdminSettings, servers: Vec<String>) -> Result<String, String> {
    Ok(format!("WASM: Would bulk deprecate {} servers", servers.len()))
}

#[cfg(all(target_arch = "wasm32", feature = "web-gui"))]
pub async fn bulk_restore_servers(_settings: &AdminSettings, servers: Vec<String>) -> Result<String, String> {
    Ok(format!("WASM: Would bulk restore {} servers", servers.len()))
}

#[cfg(all(target_arch = "wasm32", feature = "web-gui"))]
pub async fn scan_dns_resolution(_settings: &AdminSettings) -> Result<Vec<DnsResult>, String> {
    Ok(vec![
        DnsResult {
            server: "demo-server".to_string(),
            resolved: true,
            error: None,
        }
    ])
}

/// Helper function to make HTTP requests using browser's fetch API
#[cfg(all(target_arch = "wasm32", feature = "web-gui"))]
async fn fetch_json(url: &str) -> Result<String, String> {
    let window = web_sys::window().ok_or("No window object")?;
    
    let mut opts = RequestInit::new();
    opts.method("GET");
    opts.mode(RequestMode::Cors);
    
    let request = Request::new_with_str_and_init(url, &opts)
        .map_err(|e| format!("Failed to create request: {:?}", e))?;
    
    let resp_value = JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(|e| format!("Request failed: {:?}", e))?;
    
    let resp: Response = resp_value.dyn_into()
        .map_err(|e| format!("Failed to cast response: {:?}", e))?;
    
    if !resp.ok() {
        return Err(format!("HTTP error: {} {}", resp.status(), resp.status_text()));
    }
    
    let text_promise = resp.text()
        .map_err(|e| format!("Failed to get text promise: {:?}", e))?;
    
    let text_value = JsFuture::from(text_promise)
        .await
        .map_err(|e| format!("Failed to get text: {:?}", e))?;
    
    text_value.as_string()
        .ok_or("Response is not a string".to_string())
}