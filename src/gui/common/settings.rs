use dirs::home_dir;
use log::{debug, error, info};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KhmSettings {
    pub host: String,
    pub flow: String,
    pub known_hosts: String,
    pub basic_auth: String,
    pub in_place: bool,
    pub auto_sync_interval_minutes: u32,
}

impl Default for KhmSettings {
    fn default() -> Self {
        Self {
            host: String::new(),
            flow: String::new(),
            known_hosts: get_default_known_hosts_path(),
            basic_auth: String::new(),
            in_place: true,
            auto_sync_interval_minutes: 60,
        }
    }
}

/// Get default known_hosts file path based on OS
fn get_default_known_hosts_path() -> String {
    #[cfg(target_os = "windows")]
    {
        if let Ok(user_profile) = std::env::var("USERPROFILE") {
            format!("{}/.ssh/known_hosts", user_profile)
        } else {
            "~/.ssh/known_hosts".to_string()
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        "~/.ssh/known_hosts".to_string()
    }
}

/// Get configuration file path
pub fn get_config_path() -> PathBuf {
    let mut path = home_dir().expect("Could not find home directory");
    path.push(".khm");
    fs::create_dir_all(&path).ok();
    path.push("khm_config.json");
    path
}

/// Load settings from configuration file
pub fn load_settings() -> KhmSettings {
    let path = get_config_path();
    match fs::read_to_string(&path) {
        Ok(contents) => {
            let mut settings: KhmSettings = serde_json::from_str(&contents).unwrap_or_else(|e| {
                error!("Failed to parse KHM config: {}", e);
                KhmSettings::default()
            });
            
            // Fill in default known_hosts path if empty
            if settings.known_hosts.is_empty() {
                settings.known_hosts = get_default_known_hosts_path();
            }
            
            settings
        }
        Err(_) => {
            debug!("KHM config file not found, using defaults");
            KhmSettings::default()
        }
    }
}

/// Save settings to configuration file
pub fn save_settings(settings: &KhmSettings) -> Result<(), std::io::Error> {
    let path = get_config_path();
    let json = serde_json::to_string_pretty(settings)?;
    fs::write(&path, json)?;
    info!("KHM settings saved");
    Ok(())
}

/// Expand path with ~ substitution
pub fn expand_path(path: &str) -> String {
    if path.starts_with("~/") {
        if let Some(home) = home_dir() {
            return home.join(&path[2..]).to_string_lossy().to_string();
        }
    }
    path.to_string()
}

/// Perform sync operation using KHM client logic
pub async fn perform_sync(settings: &KhmSettings) -> Result<usize, std::io::Error> {
    use crate::Args;
    
    info!("Starting sync with settings: host={}, flow={}, known_hosts={}, in_place={}", 
          settings.host, settings.flow, settings.known_hosts, settings.in_place);
    
    // Convert KhmSettings to Args for client module
    let args = Args {
        server: false,
        gui: false,
        settings_ui: false,
        in_place: settings.in_place,
        flows: vec!["default".to_string()], // Not used in client mode
        ip: "127.0.0.1".to_string(),      // Not used in client mode
        port: 8080,                       // Not used in client mode
        db_host: "127.0.0.1".to_string(), // Not used in client mode
        db_name: "khm".to_string(),       // Not used in client mode
        db_user: None,                    // Not used in client mode
        db_password: None,                // Not used in client mode
        host: Some(settings.host.clone()),
        flow: Some(settings.flow.clone()),
        known_hosts: expand_path(&settings.known_hosts),
        basic_auth: settings.basic_auth.clone(),
    };
    
    info!("Expanded known_hosts path: {}", args.known_hosts);
    
    // Get keys count before and after sync
    let keys_before = crate::client::read_known_hosts(&args.known_hosts)
        .unwrap_or_else(|_| Vec::new())
        .len();
    
    crate::client::run_client(args.clone()).await?;
    
    let keys_after = if args.in_place {
        crate::client::read_known_hosts(&args.known_hosts)
            .unwrap_or_else(|_| Vec::new())
            .len()
    } else {
        keys_before
    };
    
    info!("Sync completed: {} keys before, {} keys after", keys_before, keys_after);
    Ok(keys_after)
}
