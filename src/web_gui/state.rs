use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminSettings {
    pub server_url: String,
    pub basic_auth: String,
    pub selected_flow: String,
    pub auto_refresh: bool,
    pub refresh_interval: u32,
}

impl Default for AdminSettings {
    fn default() -> Self {
        Self {
            server_url: String::new(),
            basic_auth: String::new(),
            selected_flow: String::new(),
            auto_refresh: false,
            refresh_interval: 30,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminState {
    pub keys: Vec<SshKey>,
    pub filtered_keys: Vec<SshKey>,
    pub search_term: String,
    pub show_deprecated_only: bool,
    pub selected_servers: HashMap<String, bool>,
    pub expanded_servers: HashMap<String, bool>,
    pub current_operation: String,
}

impl Default for AdminState {
    fn default() -> Self {
        Self {
            keys: Vec::new(),
            filtered_keys: Vec::new(),
            search_term: String::new(),
            show_deprecated_only: false,
            selected_servers: HashMap::new(),
            expanded_servers: HashMap::new(),
            current_operation: "Ready".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SshKey {
    pub server: String,
    pub public_key: String,
    #[serde(default)]
    pub deprecated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
    Error(String),
}

impl PartialEq for ConnectionStatus {
    fn eq(&self, other: &Self) -> bool {
        std::mem::discriminant(self) == std::mem::discriminant(other)
    }
}

#[derive(Debug, Clone)]
pub enum AdminOperation {
    LoadKeys(Result<Vec<SshKey>, String>),
    LoadFlows(Result<Vec<String>, String>),
    DeprecateKey(String, Result<String, String>),
    RestoreKey(String, Result<String, String>),
    DeleteKey(String, Result<String, String>),
    BulkDeprecate(Result<String, String>),
    BulkRestore(Result<String, String>),
    TestConnection(Result<String, String>),
    ScanDns(Result<Vec<DnsResult>, String>),
    LoadVersion(Result<String, String>),
}

// Re-export DnsResolutionResult from web.rs for consistency
pub use crate::web::DnsResolutionResult as DnsResult;

impl AdminState {
    /// Filter keys based on search term and deprecated filter
    pub fn filter_keys(&mut self) {
        let mut filtered = self.keys.clone();
        
        // Apply deprecated filter
        if self.show_deprecated_only {
            filtered.retain(|key| key.deprecated);
        }
        
        // Apply search filter
        if !self.search_term.is_empty() {
            let search_term = self.search_term.to_lowercase();
            filtered.retain(|key| {
                key.server.to_lowercase().contains(&search_term)
                    || key.public_key.to_lowercase().contains(&search_term)
            });
        }
        
        self.filtered_keys = filtered;
    }
    
    /// Get selected servers list
    pub fn get_selected_servers(&self) -> Vec<String> {
        self.selected_servers
            .iter()
            .filter_map(|(server, &selected)| {
                if selected { Some(server.clone()) } else { None }
            })
            .collect()
    }
    
    /// Clear selection
    pub fn clear_selection(&mut self) {
        self.selected_servers.clear();
    }
    
    /// Get statistics
    pub fn get_statistics(&self) -> AdminStatistics {
        let total_keys = self.keys.len();
        let active_keys = self.keys.iter().filter(|k| !k.deprecated).count();
        let deprecated_keys = total_keys - active_keys;
        let unique_servers = self.keys
            .iter()
            .map(|k| &k.server)
            .collect::<std::collections::HashSet<_>>()
            .len();
            
        AdminStatistics {
            total_keys,
            active_keys,
            deprecated_keys,
            unique_servers,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AdminStatistics {
    pub total_keys: usize,
    pub active_keys: usize,
    pub deprecated_keys: usize,
    pub unique_servers: usize,
}

/// Get SSH key type from public key string
pub fn get_key_type(public_key: &str) -> String {
    if public_key.starts_with("ssh-rsa") {
        "RSA".to_string()
    } else if public_key.starts_with("ssh-ed25519") {
        "ED25519".to_string()
    } else if public_key.starts_with("ecdsa-sha2-nistp") {
        "ECDSA".to_string()
    } else if public_key.starts_with("ssh-dss") {
        "DSA".to_string()
    } else {
        "Unknown".to_string()
    }
}

/// Get preview of SSH key (first 16 characters of key part)
pub fn get_key_preview(public_key: &str) -> String {
    let parts: Vec<&str> = public_key.split_whitespace().collect();
    if parts.len() >= 2 {
        let key_part = parts[1];
        if key_part.len() > 16 {
            format!("{}...", &key_part[..16])
        } else {
            key_part.to_string()
        }
    } else {
        format!("{}...", &public_key[..std::cmp::min(16, public_key.len())])
    }
}