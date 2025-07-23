use crate::gui::api::{fetch_keys, SshKey};
use crate::gui::common::KhmSettings;
use eframe::egui;
use log::{error, info};
use std::collections::HashMap;
use std::sync::mpsc;

#[derive(Debug, Clone)]
pub enum AdminOperation {
    LoadingKeys,
    DeprecatingKey,
    RestoringKey,
    DeletingKey,
    BulkDeprecating,
    BulkRestoring,
    None,
}

#[derive(Debug, Clone)]
pub struct AdminState {
    pub keys: Vec<SshKey>,
    pub filtered_keys: Vec<SshKey>,
    pub search_term: String,
    pub show_deprecated_only: bool,
    pub selected_servers: HashMap<String, bool>,
    pub expanded_servers: HashMap<String, bool>,
    pub current_operation: AdminOperation,
    pub last_load_time: Option<std::time::Instant>,
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
            current_operation: AdminOperation::None,
            last_load_time: None,
        }
    }
}

impl AdminState {
    /// Filter keys based on current search term and deprecated filter
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

    /// Load keys from server
    pub fn load_keys(
        &mut self,
        settings: &KhmSettings,
        ctx: &egui::Context,
    ) -> Option<mpsc::Receiver<Result<Vec<SshKey>, String>>> {
        if settings.host.is_empty() || settings.flow.is_empty() {
            return None;
        }

        self.current_operation = AdminOperation::LoadingKeys;

        let (tx, rx) = mpsc::channel();

        let host = settings.host.clone();
        let flow = settings.flow.clone();
        let basic_auth = settings.basic_auth.clone();
        let ctx_clone = ctx.clone();

        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let result = rt.block_on(async { fetch_keys(host, flow, basic_auth).await });

            let _ = tx.send(result);
            ctx_clone.request_repaint();
        });

        Some(rx)
    }

    /// Handle keys load result
    pub fn handle_keys_loaded(&mut self, result: Result<Vec<SshKey>, String>) {
        match result {
            Ok(keys) => {
                self.keys = keys;
                self.last_load_time = Some(std::time::Instant::now());
                self.filter_keys();
                self.current_operation = AdminOperation::None;
                info!("Keys loaded successfully: {} keys", self.keys.len());
            }
            Err(error) => {
                self.current_operation = AdminOperation::None;
                error!("Failed to load keys: {}", error);
            }
        }
    }

    /// Get selected servers list
    pub fn get_selected_servers(&self) -> Vec<String> {
        self.selected_servers
            .iter()
            .filter_map(|(server, &selected)| if selected { Some(server.clone()) } else { None })
            .collect()
    }

    /// Clear selected servers
    pub fn clear_selection(&mut self) {
        self.selected_servers.clear();
    }

    /// Get statistics
    pub fn get_statistics(&self) -> AdminStatistics {
        let total_keys = self.keys.len();
        let active_keys = self.keys.iter().filter(|k| !k.deprecated).count();
        let deprecated_keys = total_keys - active_keys;
        let unique_servers = self
            .keys
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

/// Get preview of SSH key (first 12 characters of key part)
pub fn get_key_preview(public_key: &str) -> String {
    let parts: Vec<&str> = public_key.split_whitespace().collect();
    if parts.len() >= 2 {
        let key_part = parts[1];
        if key_part.len() > 12 {
            format!("{}...", &key_part[..12])
        } else {
            key_part.to_string()
        }
    } else {
        format!("{}...", &public_key[..std::cmp::min(12, public_key.len())])
    }
}
