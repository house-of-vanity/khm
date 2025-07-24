// Минимальная WASM библиотека только для egui интерфейса
use wasm_bindgen::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// Основные структуры данных (копии из main lib)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SshKey {
    pub server: String,
    pub public_key: String,
    #[serde(default)]
    pub deprecated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsResult {
    pub server: String,
    pub resolved: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AdminSettings {
    pub server_url: String,
    pub basic_auth: String,
    pub selected_flow: String,
    pub auto_refresh: bool,
    pub refresh_interval: u32,
}

impl Default for AdminSettings {
    fn default() -> Self {
        let server_url = {
            #[cfg(target_arch = "wasm32")]
            {
                web_sys::window()
                    .and_then(|w| w.location().origin().ok())
                    .unwrap_or_else(|| "http://localhost:8080".to_string())
            }
            #[cfg(not(target_arch = "wasm32"))]
            {
                "http://localhost:8080".to_string()
            }
        };
        
        Self {
            server_url,
            basic_auth: String::new(),
            selected_flow: String::new(),
            auto_refresh: false,
            refresh_interval: 30,
        }
    }
}

#[derive(Debug, Clone)]
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
            current_operation: String::new(),
        }
    }
}

impl AdminState {
    pub fn filter_keys(&mut self) {
        self.filtered_keys = self.keys.iter()
            .filter(|key| {
                if self.show_deprecated_only && !key.deprecated {
                    return false;
                }
                if !self.show_deprecated_only && key.deprecated {
                    return false;
                }
                if !self.search_term.is_empty() {
                    let search_lower = self.search_term.to_lowercase();
                    return key.server.to_lowercase().contains(&search_lower) ||
                           key.public_key.to_lowercase().contains(&search_lower);
                }
                true
            })
            .cloned()
            .collect();
    }
}

// Простое egui приложение
pub struct WebAdminApp {
    settings: AdminSettings,
    admin_state: AdminState,
    status_message: String,
}

impl Default for WebAdminApp {
    fn default() -> Self {
        Self {
            settings: AdminSettings::default(),
            admin_state: AdminState::default(),
            status_message: "Ready".to_string(),
        }
    }
}

impl eframe::App for WebAdminApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("🔑 KHM Web Admin Panel");
            ui.separator();
            
            // Connection Settings
            egui::CollapsingHeader::new("⚙️ Connection Settings")
                .default_open(true)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Server URL:");
                        ui.text_edit_singleline(&mut self.settings.server_url);
                    });
                    
                    ui.horizontal(|ui| {
                        ui.label("Basic Auth:");
                        ui.add(egui::TextEdit::singleline(&mut self.settings.basic_auth).password(true));
                    });
                    
                    ui.horizontal(|ui| {
                        ui.label("Flow:");
                        ui.text_edit_singleline(&mut self.settings.selected_flow);
                    });
                    
                    ui.horizontal(|ui| {
                        if ui.button("Test Connection").clicked() {
                            self.status_message = "Testing connection... (WASM demo mode)".to_string();
                        }
                        if ui.button("Load Keys").clicked() {
                            // Add demo data
                            self.admin_state.keys = vec![
                                SshKey {
                                    server: "demo-server-1".to_string(),
                                    public_key: "ssh-rsa AAAAB3NzaC1yc2EAAAADAQABAAABgQC demo key 1".to_string(),
                                    deprecated: false,
                                },
                                SshKey {
                                    server: "demo-server-2".to_string(),
                                    public_key: "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5 demo key 2".to_string(),
                                    deprecated: true,
                                },
                            ];
                            self.admin_state.filter_keys();
                            self.status_message = format!("Loaded {} demo keys", self.admin_state.keys.len());
                        }
                    });
                });
            
            ui.add_space(10.0);
            
            // Keys display
            if !self.admin_state.filtered_keys.is_empty() {
                egui::CollapsingHeader::new("🔑 SSH Keys")
                    .default_open(true)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label("Search:");
                            let search_response = ui.text_edit_singleline(&mut self.admin_state.search_term);
                            if search_response.changed() {
                                self.admin_state.filter_keys();
                            }
                        });
                        
                        ui.horizontal(|ui| {
                            if ui.selectable_label(!self.admin_state.show_deprecated_only, "✅ Active").clicked() {
                                self.admin_state.show_deprecated_only = false;
                                self.admin_state.filter_keys();
                            }
                            if ui.selectable_label(self.admin_state.show_deprecated_only, "❗ Deprecated").clicked() {
                                self.admin_state.show_deprecated_only = true;
                                self.admin_state.filter_keys();
                            }
                        });
                        
                        ui.separator();
                        
                        for key in &self.admin_state.filtered_keys {
                            ui.group(|ui| {
                                ui.horizontal(|ui| {
                                    if key.deprecated {
                                        ui.colored_label(egui::Color32::RED, "❗ DEPRECATED");
                                    } else {
                                        ui.colored_label(egui::Color32::GREEN, "✅ ACTIVE");
                                    }
                                    
                                    ui.label(&key.server);
                                    ui.monospace(&key.public_key[..50.min(key.public_key.len())]);
                                    
                                    if ui.small_button("Copy").clicked() {
                                        ui.output_mut(|o| o.copied_text = key.public_key.clone());
                                    }
                                });
                            });
                        }
                    });
            }
            
            ui.add_space(10.0);
            
            // Status
            ui.horizontal(|ui| {
                ui.label("Status:");
                ui.colored_label(egui::Color32::LIGHT_BLUE, &self.status_message);
            });
            
            // Info
            ui.separator();
            ui.label("ℹ️ This is a demo WASM version. For full functionality, the server API integration is needed.");
        });
    }
}

/// WASM entry point
#[wasm_bindgen]
pub fn start_web_admin(canvas_id: &str) -> Result<(), JsValue> {
    console_error_panic_hook::set_once();
    tracing_wasm::set_as_global_default();
    
    let web_options = eframe::WebOptions::default();
    let canvas_id = canvas_id.to_string();
    
    wasm_bindgen_futures::spawn_local(async move {
        let app = WebAdminApp::default();
        
        let result = eframe::WebRunner::new()
            .start(
                &canvas_id,
                web_options,
                Box::new(|_cc| Ok(Box::new(app))),
            )
            .await;
            
        match result {
            Ok(_) => web_sys::console::log_1(&"eframe started successfully".into()),
            Err(e) => web_sys::console::error_1(&format!("Failed to start eframe: {:?}", e).into()),
        }
    });
    
    Ok(())
}

#[wasm_bindgen(start)]
pub fn wasm_main() {
    console_error_panic_hook::set_once();
}