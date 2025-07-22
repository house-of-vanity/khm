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
}

impl Default for KhmSettings {
    fn default() -> Self {
        Self {
            host: String::new(),
            flow: String::new(),
            known_hosts: "~/.ssh/known_hosts".to_string(),
            basic_auth: String::new(),
            in_place: false,
        }
    }
}

pub fn get_config_path() -> PathBuf {
    let mut path = home_dir().expect("Could not find home directory");
    path.push(".khm");
    fs::create_dir_all(&path).ok();
    path.push("khm_config.json");
    path
}

pub fn load_settings() -> KhmSettings {
    let path = get_config_path();
    match fs::read_to_string(&path) {
        Ok(contents) => serde_json::from_str(&contents).unwrap_or_else(|e| {
            error!("Failed to parse KHM config: {}", e);
            KhmSettings::default()
        }),
        Err(_) => {
            debug!("KHM config file not found, using defaults");
            KhmSettings::default()
        }
    }
}

pub fn save_settings(settings: &KhmSettings) -> Result<(), std::io::Error> {
    let path = get_config_path();
    let json = serde_json::to_string_pretty(settings)?;
    fs::write(&path, json)?;
    info!("KHM settings saved");
    Ok(())
}

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::run_settings_window;

#[cfg(not(target_os = "macos"))]
mod cross;
#[cfg(not(target_os = "macos"))]
pub use cross::run_settings_window;

// Helper function to expand tilde in path
pub fn expand_path(path: &str) -> String {
    if path.starts_with("~/") {
        if let Some(home) = home_dir() {
            return home.join(&path[2..]).to_string_lossy().to_string();
        }
    }
    path.to_string()
}
