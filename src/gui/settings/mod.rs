use dirs::home_dir;
use eframe::egui;
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
            known_hosts: "~/.ssh/known_hosts".to_string(),
            basic_auth: String::new(),
            in_place: true,
            auto_sync_interval_minutes: 60,
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

pub fn expand_path(path: &str) -> String {
    if path.starts_with("~/") {
        if let Some(home) = home_dir() {
            return home.join(&path[2..]).to_string_lossy().to_string();
        }
    }
    path.to_string()
}

struct KhmSettingsWindow {
    settings: KhmSettings,
    auto_sync_interval_str: String,
}

impl eframe::App for KhmSettingsWindow {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("KHM Settings");
            ui.separator();
            
            ui.horizontal(|ui| {
                ui.label("Host URL:");
                ui.text_edit_singleline(&mut self.settings.host);
            });
            
            ui.horizontal(|ui| {
                ui.label("Flow Name:");
                ui.text_edit_singleline(&mut self.settings.flow);
            });
            
            ui.horizontal(|ui| {
                ui.label("Known Hosts:");
                ui.text_edit_singleline(&mut self.settings.known_hosts);
            });
            
            ui.horizontal(|ui| {
                ui.label("Basic Auth:");
                ui.text_edit_singleline(&mut self.settings.basic_auth);
            });
            
            ui.horizontal(|ui| {
                ui.label("Auto sync interval (min):");
                ui.text_edit_singleline(&mut self.auto_sync_interval_str);
                // Parse the string and update settings
                if let Ok(value) = self.auto_sync_interval_str.parse::<u32>() {
                    self.settings.auto_sync_interval_minutes = value;
                }
            });
            
            ui.checkbox(&mut self.settings.in_place, "Update known_hosts file in-place after sync");
            
            ui.separator();
            
            ui.horizontal(|ui| {
                if ui.button("Save").clicked() {
                    if let Err(e) = save_settings(&self.settings) {
                        error!("Failed to save KHM settings: {}", e);
                    } else {
                        info!("KHM settings saved successfully");
                    }
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                
                if ui.button("Cancel").clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
        });
    }
}

pub fn run_settings_window() {
    let settings = load_settings();
    let auto_sync_interval_str = settings.auto_sync_interval_minutes.to_string();
    
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("KHM Settings")
            .with_inner_size([450.0, 385.0]),
        ..Default::default()
    };
    
    let _ = eframe::run_native(
        "KHM Settings",
        options,
        Box::new(|_cc| Ok(Box::new(KhmSettingsWindow { 
            settings,
            auto_sync_interval_str,
        }))),
    );
}
