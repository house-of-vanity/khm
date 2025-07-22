use dirs::home_dir;
use eframe::egui;
use log::{debug, error, info};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::mpsc;

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
    config_content: String,
    connection_status: ConnectionStatus,
    is_testing_connection: bool,
    test_result_receiver: Option<mpsc::Receiver<Result<String, String>>>,
}

#[derive(Debug, Clone)]
enum ConnectionStatus {
    Unknown,
    Connected { keys_count: usize, flow: String },
    Error(String),
}

impl eframe::App for KhmSettingsWindow {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Check for test connection result
        if let Some(receiver) = &self.test_result_receiver {
            if let Ok(result) = receiver.try_recv() {
                self.is_testing_connection = false;
                match result {
                    Ok(message) => {
                        // Parse keys count from message
                        let keys_count = if let Some(start) = message.find("Found ") {
                            if let Some(end) = message[start + 6..].find(" SSH keys") {
                                message[start + 6..start + 6 + end].parse::<usize>().unwrap_or(0)
                            } else { 0 }
                        } else { 0 };
                        
                        let flow = self.settings.flow.clone();
                        self.connection_status = ConnectionStatus::Connected { keys_count, flow };
                        info!("Connection test successful: {}", message);
                    }
                    Err(error) => {
                        self.connection_status = ConnectionStatus::Error(error.clone());
                        error!("Connection test failed: {}", error);
                    }
                }
                self.test_result_receiver = None;
                ctx.request_repaint();
            }
        }
        // Apply modern dark theme
        ctx.set_visuals(egui::Visuals {
            button_frame: true,
            collapsing_header_frame: true,
            indent_has_left_vline: true,
            ..egui::Visuals::dark()
        });
        
        egui::CentralPanel::default()
            .frame(egui::Frame::none().inner_margin(egui::Margin::same(20.0)))
            .show(ctx, |ui| {
                // Header with title
                ui.horizontal(|ui| {
                    ui.heading(egui::RichText::new("🔐 KHM Settings").size(24.0));
                });
                
                ui.add_space(10.0);
                ui.separator();
                ui.add_space(15.0);
                
                // Connection section
                ui.group(|ui| {
                    ui.set_min_width(ui.available_width());
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("🌐 Connection").size(16.0).strong());
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                let mut connected = matches!(self.connection_status, ConnectionStatus::Connected { .. });
                                ui.add_enabled(false, egui::Checkbox::new(&mut connected, "Connected"));
                                
                                if self.is_testing_connection {
                                    ui.spinner();
                                    ui.label(egui::RichText::new("Testing...").italics());
                                } else {
                                    match &self.connection_status {
                                        ConnectionStatus::Unknown => {
                                            ui.label(egui::RichText::new("Not tested").color(egui::Color32::GRAY));
                                        }
                                        ConnectionStatus::Connected { keys_count, flow } => {
                                            ui.label(egui::RichText::new("✓").color(egui::Color32::GREEN));
                                            ui.label(egui::RichText::new(format!("{} keys in '{}'", keys_count, flow))
                                                .color(egui::Color32::LIGHT_GREEN));
                                        }
                                        ConnectionStatus::Error(err) => {
                                            ui.label(egui::RichText::new("✗").color(egui::Color32::RED))
                                                .on_hover_text(format!("Error: {}", err));
                                            ui.label(egui::RichText::new("Failed").color(egui::Color32::RED));
                                        }
                                    }
                                }
                            });
                        });
                        
                        ui.add_space(8.0);
                        
                        egui::Grid::new("connection_grid")
                            .num_columns(2)
                            .min_col_width(120.0)
                            .spacing([10.0, 8.0])
                            .show(ui, |ui| {
                                ui.label("Host URL:");
                                ui.add_sized(
                                    [ui.available_width(), 20.0],
                                    egui::TextEdit::singleline(&mut self.settings.host)
                                        .hint_text("https://your-khm-server.com")
                                );
                                ui.end_row();
                                
                                ui.label("Flow Name:");
                                ui.add_sized(
                                    [ui.available_width(), 20.0],
                                    egui::TextEdit::singleline(&mut self.settings.flow)
                                        .hint_text("production, staging, etc.")
                                );
                                ui.end_row();
                                
                                ui.label("Basic Auth:");
                                ui.add_sized(
                                    [ui.available_width(), 20.0],
                                    egui::TextEdit::singleline(&mut self.settings.basic_auth)
                                        .hint_text("username:password (optional)")
                                        .password(true)
                                );
                                ui.end_row();
                            });
                    });
                });
                
                ui.add_space(15.0);
                
                // Local settings section
                ui.group(|ui| {
                    ui.set_min_width(ui.available_width());
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new("📁 Local Settings").size(16.0).strong());
                        ui.add_space(8.0);
                        
                        egui::Grid::new("local_grid")
                            .num_columns(2)
                            .min_col_width(120.0)
                            .spacing([10.0, 8.0])
                            .show(ui, |ui| {
                                ui.label("Known Hosts File:");
                                ui.add_sized(
                                    [ui.available_width(), 20.0],
                                    egui::TextEdit::singleline(&mut self.settings.known_hosts)
                                        .hint_text("~/.ssh/known_hosts")
                                );
                                ui.end_row();
                            });
                        
                        ui.add_space(8.0);
                        ui.checkbox(&mut self.settings.in_place, "✏️ Update known_hosts file in-place after sync");
                    });
                });
                
                ui.add_space(15.0);
                
                // Auto-sync section
                ui.group(|ui| {
                    ui.set_min_width(ui.available_width());
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new("🔄 Auto Sync").size(16.0).strong());
                        ui.add_space(8.0);
                        
                        let is_auto_sync_enabled = !self.settings.host.is_empty() 
                            && !self.settings.flow.is_empty() 
                            && self.settings.in_place;
                        
                        ui.horizontal(|ui| {
                            ui.label("Interval (minutes):");
                            ui.add_sized(
                                [80.0, 20.0],
                                egui::TextEdit::singleline(&mut self.auto_sync_interval_str)
                            );
                            
                            if let Ok(value) = self.auto_sync_interval_str.parse::<u32>() {
                                if value > 0 {
                                    self.settings.auto_sync_interval_minutes = value;
                                }
                            }
                            
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if is_auto_sync_enabled {
                                    ui.label(egui::RichText::new("✅ Enabled").color(egui::Color32::GREEN));
                                } else {
                                    ui.label(egui::RichText::new("⏸️ Disabled").color(egui::Color32::YELLOW));
                                    ui.label("(Configure host, flow & enable in-place sync)");
                                }
                            });
                        });
                    });
                });
                
                ui.add_space(15.0);
                
                // Advanced settings (collapsible)
                ui.collapsing("🔧 Advanced Settings", |ui| {
                    ui.indent("advanced", |ui| {
                        ui.label("Configuration details:");
                        ui.add_space(5.0);
                        
                        ui.horizontal(|ui| {
                            ui.label("Config file:");
                            let config_path = get_config_path();
                            ui.label(egui::RichText::new(config_path.display().to_string())
                                .font(egui::FontId::monospace(12.0))
                                .color(egui::Color32::LIGHT_GRAY));
                        });
                        
                        ui.add_space(8.0);
                        ui.label("Current configuration:");
                        
                        ui.add_sized(
                            [ui.available_width(), 120.0],
                            egui::TextEdit::multiline(&mut self.config_content.clone())
                                .font(egui::FontId::monospace(11.0))
                                .interactive(false)
                        );
                    });
                });
                
                ui.add_space(20.0);
                ui.separator();
                ui.add_space(15.0);
                
                // Action buttons
                let save_enabled = !self.settings.host.is_empty() && !self.settings.flow.is_empty();
                
                ui.horizontal(|ui| {
                    if ui.add_enabled(
                        save_enabled,
                        egui::Button::new("💾 Save Settings")
                            .min_size(egui::vec2(120.0, 32.0))
                    ).clicked() {
                        if let Err(e) = save_settings(&self.settings) {
                            error!("Failed to save KHM settings: {}", e);
                        } else {
                            info!("KHM settings saved successfully");
                        }
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                    
                    if ui.add(
                        egui::Button::new("❌ Cancel")
                            .min_size(egui::vec2(80.0, 32.0))
                    ).clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                    
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let can_test = !self.settings.host.is_empty() && !self.settings.flow.is_empty() && !self.is_testing_connection;
                        
                        if ui.add_enabled(
                            can_test,
                            egui::Button::new(
                                if self.is_testing_connection {
                                    "🔄 Testing..."
                                } else {
                                    "🧪 Test Connection"
                                }
                            ).min_size(egui::vec2(120.0, 32.0))
                        ).clicked() {
                            self.start_connection_test(ctx);
                        }
                    });
                });
                
                // Show validation hints
                if !save_enabled {
                    ui.add_space(5.0);
                    ui.label(egui::RichText::new("⚠️ Please fill in Host URL and Flow Name to save settings")
                        .color(egui::Color32::YELLOW)
                        .italics());
                }
            });
    }
}

pub fn run_settings_window() {
    let settings = load_settings();
    let auto_sync_interval_str = settings.auto_sync_interval_minutes.to_string();
    
    // Load config file content
    let config_content = match fs::read_to_string(get_config_path()) {
        Ok(content) => content,
        Err(_) => "Configuration file not found or empty".to_string(),
    };
    
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("KHM Settings")
            .with_inner_size([520.0, 750.0])
            .with_min_inner_size([480.0, 600.0])
            .with_resizable(true)
            .with_icon(create_window_icon()),
        ..Default::default()
    };
    
    let _ = eframe::run_native(
        "KHM Settings",
        options,
        Box::new(|_cc| Ok(Box::new(KhmSettingsWindow { 
            settings,
            auto_sync_interval_str,
            config_content,
            connection_status: ConnectionStatus::Unknown,
            is_testing_connection: false,
            test_result_receiver: None,
        }))),
    );
}

fn create_window_icon() -> egui::IconData {
    // Create a simple programmatic icon (blue square with white border)
    let icon_size = 32;
    let icon_data: Vec<u8> = (0..icon_size * icon_size)
        .flat_map(|i| {
            let y = i / icon_size;
            let x = i % icon_size;
            if x < 2 || x >= 30 || y < 2 || y >= 30 {
                [255, 255, 255, 255] // White border
            } else {
                [64, 128, 255, 255] // Blue center
            }
        })
        .collect();
    
    egui::IconData {
        rgba: icon_data,
        width: icon_size as u32,
        height: icon_size as u32,
    }
}

impl KhmSettingsWindow {
    fn start_connection_test(&mut self, ctx: &egui::Context) {
        if self.is_testing_connection {
            return;
        }
        
        self.is_testing_connection = true;
        self.connection_status = ConnectionStatus::Unknown;
        
        let (tx, rx) = mpsc::channel();
        self.test_result_receiver = Some(rx);
        
        let host = self.settings.host.clone();
        let flow = self.settings.flow.clone();
        let basic_auth = self.settings.basic_auth.clone();
        let ctx_clone = ctx.clone();
        
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let result = rt.block_on(async {
                test_khm_connection(host, flow, basic_auth).await
            });
            
            let _ = tx.send(result);
            ctx_clone.request_repaint();
        });
    }
}

async fn test_khm_connection(host: String, flow: String, basic_auth: String) -> Result<String, String> {
    if host.is_empty() || flow.is_empty() {
        return Err("Host and flow must be specified".to_string());
    }
    
    let url = format!("{}/{}/keys", host.trim_end_matches('/'), flow);
    info!("Testing connection to: {}", url);
    
    let client_builder = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10));
    
    let client = client_builder.build().map_err(|e| format!("Failed to create HTTP client: {}", e))?;
    
    let mut request = client.get(&url);
    
    // Add basic auth if provided
    if !basic_auth.is_empty() {
        let auth_parts: Vec<&str> = basic_auth.splitn(2, ':').collect();
        if auth_parts.len() == 2 {
            request = request.basic_auth(auth_parts[0], Some(auth_parts[1]));
        } else {
            return Err("Basic auth format should be 'username:password'".to_string());
        }
    }
    
    let response = request.send().await
        .map_err(|e| format!("Connection failed: {}", e))?;
    
    if !response.status().is_success() {
        return Err(format!("Server returned error: {} {}", response.status().as_u16(), response.status().canonical_reason().unwrap_or("Unknown")));
    }
    
    let body = response.text().await
        .map_err(|e| format!("Failed to read response: {}", e))?;
    
    // Parse JSON response to count SSH keys
    if body.trim().is_empty() {
        return Err("Server returned empty response".to_string());
    }
    
    // Try to parse as JSON array
    let keys: Result<Vec<serde_json::Value>, _> = serde_json::from_str(&body);
    
    match keys {
        Ok(key_array) => {
            let ssh_key_count = key_array.len();
            if ssh_key_count == 0 {
                return Err("No SSH keys found in response".to_string());
            }
            Ok(format!("Connection successful! Found {} SSH keys in flow '{}'", ssh_key_count, flow))
        }
        Err(_) => {
            // Fallback: try to parse as plain text (old format)
            let lines: Vec<&str> = body.lines().collect();
            let ssh_key_count = lines.iter()
                .filter(|line| !line.trim().is_empty() && !line.starts_with('#'))
                .count();
            
            if ssh_key_count == 0 {
                return Err("Invalid response format - not JSON array or SSH keys text".to_string());
            }
            
            Ok(format!("Connection successful! Found {} SSH keys in flow '{}'", ssh_key_count, flow))
        }
    }
}
