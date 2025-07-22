use dirs::home_dir;
use eframe::egui;
use log::{debug, error, info};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::mpsc;
use reqwest::Client;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KhmSettings {
    pub host: String,
    pub flow: String,
    pub known_hosts: String,
    pub basic_auth: String,
    pub in_place: bool,
    pub auto_sync_interval_minutes: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SshKey {
    pub server: String,
    pub public_key: String,
    #[serde(default)]
    pub deprecated: bool,
}

#[derive(Debug, Clone)]
enum AdminOperation {
    LoadingKeys,
    DeprecatingKey,
    RestoringKey, 
    DeletingKey,
    BulkDeprecating,
    BulkRestoring,
    None,
}

#[derive(Debug, Clone)]
struct AdminState {
    keys: Vec<SshKey>,
    filtered_keys: Vec<SshKey>,
    search_term: String,
    show_deprecated_only: bool,
    selected_servers: HashMap<String, bool>,
    expanded_servers: HashMap<String, bool>,
    current_operation: AdminOperation,
    last_load_time: Option<std::time::Instant>,
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

async fn fetch_admin_keys(host: String, flow: String, basic_auth: String) -> Result<Vec<SshKey>, String> {
    if host.is_empty() || flow.is_empty() {
        return Err("Host and flow must be specified".to_string());
    }
    
    let url = format!("{}/{}/keys?include_deprecated=true", host.trim_end_matches('/'), flow);
    info!("Fetching admin keys from: {}", url);
    
    let client_builder = Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::none()); // Don't follow redirects
    
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
        .map_err(|e| format!("Request failed: {}", e))?;
    
    // Check for authentication required
    if response.status().as_u16() == 401 {
        return Err("Authentication required. Please provide valid basic auth credentials.".to_string());
    }
    
    // Check for redirects (usually to login page)
    if response.status().as_u16() >= 300 && response.status().as_u16() < 400 {
        return Err("Server redirects to login page. Authentication may be required.".to_string());
    }
    
    if !response.status().is_success() {
        return Err(format!("Server returned error: {} {}", response.status().as_u16(), response.status().canonical_reason().unwrap_or("Unknown")));
    }
    
    let body = response.text().await
        .map_err(|e| format!("Failed to read response: {}", e))?;
    
    // Check if response looks like HTML (login page)
    if body.trim_start().starts_with("<!DOCTYPE") || body.trim_start().starts_with("<html") {
        return Err("Server returned HTML page instead of JSON. This usually means authentication is required or the endpoint is incorrect.".to_string());
    }
    
    let keys: Vec<SshKey> = serde_json::from_str(&body)
        .map_err(|e| format!("Failed to parse response: {}", e))?;
    
    info!("Fetched {} SSH keys", keys.len());
    Ok(keys)
}

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
    admin_state: AdminState,
    admin_receiver: Option<mpsc::Receiver<Result<Vec<SshKey>, String>>>,
    operation_receiver: Option<mpsc::Receiver<Result<String, String>>>,
    current_tab: SettingsTab,
    is_syncing: bool,
    sync_result_receiver: Option<mpsc::Receiver<Result<String, String>>>,
    sync_status: SyncStatus,
}

#[derive(Debug, Clone, PartialEq)]
enum SettingsTab {
    Connection,
    Admin,
}

#[derive(Debug, Clone)]
enum ConnectionStatus {
    Unknown,
    Connected { keys_count: usize, flow: String },
    Error(String),
}

#[derive(Debug, Clone)]
enum SyncStatus {
    Unknown,
    Success { keys_count: usize },
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
        
        // Check for admin operation results
        if let Some(receiver) = &self.admin_receiver {
            if let Ok(result) = receiver.try_recv() {
                match result {
                    Ok(keys) => {
                        self.admin_state.keys = keys;
                        self.admin_state.last_load_time = Some(std::time::Instant::now());
                        self.filter_admin_keys();
                        self.admin_state.current_operation = AdminOperation::None;
                        info!("Keys loaded successfully: {} keys", self.admin_state.keys.len());
                    }
                    Err(error) => {
                        self.admin_state.current_operation = AdminOperation::None;
                        error!("Failed to load keys: {}", error);
                    }
                }
                self.admin_receiver = None;
                ctx.request_repaint();
            }
        }
        
        // Check for sync result
        if let Some(receiver) = &self.sync_result_receiver {
            if let Ok(result) = receiver.try_recv() {
                self.is_syncing = false;
                match result {
                    Ok(message) => {
                        info!("Parsing sync result message: '{}'", message);
                        
                        // Parse keys count from message - fix parsing patterns
                        let keys_count = if let Some(start) = message.find("updated with ") {
                            let search_start = start + "updated with ".len();
                            if let Some(end) = message[search_start..].find(" keys") {
                                let number_str = &message[search_start..search_start + end];
                                info!("Found 'updated with' pattern, parsing: '{}'", number_str);
                                number_str.parse::<usize>().unwrap_or(0)
                            } else { 0 }
                        } else if let Some(start) = message.find("Retrieved ") {
                            let search_start = start + "Retrieved ".len();
                            if let Some(end) = message[search_start..].find(" keys") {
                                let number_str = &message[search_start..search_start + end];
                                info!("Found 'Retrieved' pattern, parsing: '{}'", number_str);
                                number_str.parse::<usize>().unwrap_or(0)
                            } else { 0 }
                        } else {
                            // Try to extract any number followed by "keys" in the message
                            if let Some(keys_pos) = message.find(" keys") {
                                let before_keys = &message[..keys_pos];
                                // Find the last number in the string before "keys"
                                if let Some(space_pos) = before_keys.rfind(' ') {
                                    let number_str = &before_keys[space_pos + 1..];
                                    info!("Found fallback pattern, parsing: '{}'", number_str);
                                    number_str.parse::<usize>().unwrap_or(0)
                                } else {
                                    0
                                }
                            } else {
                                0
                            }
                        };
                        
                        info!("Parsed keys count: {}", keys_count);
                        self.sync_status = SyncStatus::Success { keys_count };
                        info!("Sync successful: {}", message);
                    }
                    Err(error) => {
                        self.sync_status = SyncStatus::Error(error.clone());
                        error!("Sync failed: {}", error);
                    }
                }
                self.sync_result_receiver = None;
                ctx.request_repaint();
            }
        }
        // Check for operation results
        if let Some(receiver) = &self.operation_receiver {
            if let Ok(result) = receiver.try_recv() {
                match result {
                    Ok(message) => {
                        info!("Operation completed: {}", message);
                        // Reload keys after operation
                        self.load_admin_keys(ctx);
                    }
                    Err(error) => {
                        error!("Operation failed: {}", error);
                    }
                }
                self.admin_state.current_operation = AdminOperation::None;
                self.operation_receiver = None;
                ctx.request_repaint();
            }
        }
        
        // Apply enhanced modern dark theme for admin interface
        let mut visuals = egui::Visuals::dark();
        visuals.window_fill = egui::Color32::from_gray(25);
        visuals.panel_fill = egui::Color32::from_gray(30);
        visuals.faint_bg_color = egui::Color32::from_gray(35);
        visuals.extreme_bg_color = egui::Color32::from_gray(15);
        visuals.button_frame = true;
        visuals.collapsing_header_frame = true;
        visuals.indent_has_left_vline = true;
        visuals.menu_rounding = egui::Rounding::same(8.0);
        visuals.window_rounding = egui::Rounding::same(12.0);
        visuals.widgets.noninteractive.rounding = egui::Rounding::same(6.0);
        visuals.widgets.inactive.rounding = egui::Rounding::same(6.0);
        visuals.widgets.hovered.rounding = egui::Rounding::same(6.0);
        visuals.widgets.active.rounding = egui::Rounding::same(6.0);
        ctx.set_visuals(visuals);
        
        egui::CentralPanel::default()
            .frame(egui::Frame::none().inner_margin(egui::Margin::same(20.0)))
            .show(ctx, |ui| {
                // Header with title
                ui.horizontal(|ui| {
                    ui.heading(egui::RichText::new("🔑 KHM Settings").size(24.0));
                });
                
                ui.add_space(10.0);
                
                // Tab selector
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.current_tab, SettingsTab::Connection, "📃 Settings");
                    ui.selectable_value(&mut self.current_tab, SettingsTab::Admin, "🔧 Admin");
                });
                
                ui.separator();
                ui.add_space(15.0);
                
                match self.current_tab {
                    SettingsTab::Connection => self.render_connection_tab(ui, ctx),
                    SettingsTab::Admin => self.render_admin_tab(ui, ctx),
                }
            });
    }
}

impl KhmSettingsWindow {
    fn render_connection_tab(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let available_height = ui.available_height();
        let button_area_height = 120.0; // Reserve space for buttons and status
        let content_height = available_height - button_area_height;
        
        // Main content area (scrollable)
        ui.allocate_ui_with_layout(
            [ui.available_width(), content_height].into(),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false; 2])
                    .show(ui, |ui| {
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
                                                    ui.label(egui::RichText::new("✅").color(egui::Color32::GREEN));
                                                    ui.label(egui::RichText::new(format!("{} keys in '{}'", keys_count, flow))
                                                        .color(egui::Color32::LIGHT_GREEN));
                                                }
                                                ConnectionStatus::Error(err) => {
                                                    ui.label(egui::RichText::new("❌").color(egui::Color32::RED))
                                                        .on_hover_text(format!("Error: {}", err));
                                                    ui.label(egui::RichText::new("Failed").color(egui::Color32::RED));
                                                }
                                            }
                                        }
                                    });
                                });
                                
                                ui.add_space(5.0);
                                
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
                        
                        ui.add_space(10.0);
                        
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
                                ui.checkbox(&mut self.settings.in_place, "✏ Update known_hosts file in-place after sync");
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
                                            ui.label(egui::RichText::new("🔄 Enabled").color(egui::Color32::GREEN));
                                        } else {
                                            ui.label(egui::RichText::new("❌ Disabled").color(egui::Color32::YELLOW));
                                            ui.label("(Configure host, flow & enable in-place sync)");
                                        }
                                    });
                                });
                            });
                        });
                        
                        ui.add_space(10.0);
                        
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
                    });
            },
        );
        
        // Bottom area with buttons (fixed position)
        ui.allocate_ui_with_layout(
            [ui.available_width(), button_area_height].into(),
            egui::Layout::bottom_up(egui::Align::Min),
            |ui| {
                // Show sync status
                match &self.sync_status {
                    SyncStatus::Success { keys_count } => {
                        ui.label(egui::RichText::new(format!("✅ Last sync successful: {} keys", keys_count))
                            .color(egui::Color32::GREEN));
                    }
                    SyncStatus::Error(err) => {
                        ui.label(egui::RichText::new(format!("❌ Sync failed: {}", err))
                            .color(egui::Color32::RED));
                    }
                    SyncStatus::Unknown => {}
                }
                
                // Show validation hints
                let save_enabled = !self.settings.host.is_empty() && !self.settings.flow.is_empty();
                if !save_enabled {
                    ui.label(egui::RichText::new("❗ Please fill in Host URL and Flow Name to save settings")
                        .color(egui::Color32::YELLOW)
                        .italics());
                }
                
                ui.add_space(5.0);
                
                // Action buttons
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
                        egui::Button::new("✖ Cancel")
                            .min_size(egui::vec2(80.0, 32.0))
                    ).clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                    
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let can_test = !self.settings.host.is_empty() && !self.settings.flow.is_empty() && !self.is_testing_connection;
                        let can_sync = !self.settings.host.is_empty() && !self.settings.flow.is_empty() && !self.is_syncing;
                        
                        if ui.add_enabled(
                            can_test,
                            egui::Button::new(
                                if self.is_testing_connection {
                                    "▶ Testing..."
                                } else {
                                    "🔍 Test Connection"
                                }
                            ).min_size(egui::vec2(120.0, 32.0))
                        ).clicked() {
                            self.start_connection_test(ctx);
                        }
                        
                        if ui.add_enabled(
                            can_sync,
                            egui::Button::new(
                                if self.is_syncing {
                                    "🔄 Syncing..."
                                } else {
                                    "🔄 Sync Now"
                                }
                            ).min_size(egui::vec2(100.0, 32.0))
                        ).clicked() {
                            self.start_manual_sync(ctx);
                        }
                    });
                });
            },
        );
    }

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
    
    fn start_manual_sync(&mut self, ctx: &egui::Context) {
        if self.is_syncing {
            return;
        }
        
        self.is_syncing = true;
        self.sync_status = SyncStatus::Unknown;
        
        let (tx, rx) = mpsc::channel();
        self.sync_result_receiver = Some(rx);
        
        let settings = self.settings.clone();
        let ctx_clone = ctx.clone();
        
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let result = rt.block_on(async {
                perform_manual_sync(settings).await
            });
            
            let _ = tx.send(result);
            ctx_clone.request_repaint();
        });
    }
    
    fn render_admin_tab(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        // Admin tab header
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("🔧 Admin Panel").size(18.0).strong());
            
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("🔁 Refresh").clicked() {
                    self.load_admin_keys(ctx);
                }
                
                if let Some(last_load) = self.admin_state.last_load_time {
                    let elapsed = last_load.elapsed().as_secs();
                    ui.label(format!("Updated {}s ago", elapsed));
                }
            });
        });
        
        ui.separator();
        ui.add_space(10.0);
        
        // Check if connection is configured
        if self.settings.host.is_empty() || self.settings.flow.is_empty() {
            ui.vertical_centered(|ui| {
                ui.label(egui::RichText::new("❗ Please configure connection settings first")
                    .size(16.0)
                    .color(egui::Color32::YELLOW));
                ui.add_space(10.0);
                if ui.button("Go to Connection Settings").clicked() {
                    self.current_tab = SettingsTab::Connection;
                }
            });
            return;
        }
        
        // Load keys automatically on first view
        if self.admin_state.keys.is_empty() && !matches!(self.admin_state.current_operation, AdminOperation::LoadingKeys) {
            self.load_admin_keys(ctx);
        }
        
        // Show loading state
        if matches!(self.admin_state.current_operation, AdminOperation::LoadingKeys) {
            ui.vertical_centered(|ui| {
                ui.spinner();
                ui.label("Loading keys...");
            });
            return;
        }
        
        // Statistics cards - адаптивные как в Connection
        ui.group(|ui| {
            ui.set_min_width(ui.available_width());
            ui.vertical(|ui| {
                ui.label(egui::RichText::new("📊 Statistics").size(16.0).strong());
                ui.add_space(8.0);
                
                let total_keys = self.admin_state.keys.len();
                let active_keys = self.admin_state.keys.iter().filter(|k| !k.deprecated).count();
                let deprecated_keys = total_keys - active_keys;
                let unique_servers = self.admin_state.keys.iter().map(|k| &k.server).collect::<std::collections::HashSet<_>>().len();
                
                ui.horizontal(|ui| {
                    ui.columns(4, |cols| {
                        // Total keys
                        cols[0].vertical_centered_justified(|ui| {
                            ui.label(egui::RichText::new("📊").size(20.0));
                            ui.label(egui::RichText::new(total_keys.to_string()).size(24.0).strong());
                            ui.label(egui::RichText::new("Total Keys").size(11.0).color(egui::Color32::GRAY));
                        });
                        
                        // Active keys
                        cols[1].vertical_centered_justified(|ui| {
                            ui.label(egui::RichText::new("✅").size(20.0));
                            ui.label(egui::RichText::new(active_keys.to_string()).size(24.0).strong().color(egui::Color32::LIGHT_GREEN));
                            ui.label(egui::RichText::new("Active").size(11.0).color(egui::Color32::GRAY));
                        });
                        
                        // Deprecated keys
                        cols[2].vertical_centered_justified(|ui| {
                            ui.label(egui::RichText::new("❌").size(20.0));
                            ui.label(egui::RichText::new(deprecated_keys.to_string()).size(24.0).strong().color(egui::Color32::LIGHT_RED));
                            ui.label(egui::RichText::new("Deprecated").size(11.0).color(egui::Color32::GRAY));
                        });
                        
                        // Servers
                        cols[3].vertical_centered_justified(|ui| {
                            ui.label(egui::RichText::new("💻").size(20.0));
                            ui.label(egui::RichText::new(unique_servers.to_string()).size(24.0).strong().color(egui::Color32::LIGHT_BLUE));
                            ui.label(egui::RichText::new("Servers").size(11.0).color(egui::Color32::GRAY));
                        });
                    });
                });
            });
        });
        
        ui.add_space(10.0);
        
        // Enhanced search and filters - адаптивный подход как в блоках статистики
        ui.group(|ui| {
            ui.set_min_width(ui.available_width());
            ui.vertical(|ui| {
                ui.label(egui::RichText::new("🔍 Search").size(16.0).strong());
                ui.add_space(8.0);
                
                // Search field with full width like statistics blocks
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("🔍").size(14.0));
                    let search_response = ui.add_sized(
                        [ui.available_width() * 0.6, 20.0],
                        egui::TextEdit::singleline(&mut self.admin_state.search_term)
                            .hint_text("Search servers or keys...")
                    );
                    
                    if self.admin_state.search_term.is_empty() {
                        ui.label(egui::RichText::new("Type to search").size(11.0).color(egui::Color32::GRAY));
                    } else {
                        ui.label(egui::RichText::new(format!("{} results", self.admin_state.filtered_keys.len())).size(11.0));
                        if ui.add(egui::Button::new(egui::RichText::new("❌").color(egui::Color32::WHITE))
                            .fill(egui::Color32::from_rgb(170, 170, 170))
                            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(89, 89, 89)))
                            .rounding(egui::Rounding::same(3.0))
                            .min_size(egui::vec2(18.0, 18.0))
                        ).on_hover_text("Clear search").clicked() {
                            self.admin_state.search_term.clear();
                            self.filter_admin_keys();
                        }
                    }
                    
                    // Handle search text changes
                    if search_response.changed() {
                        self.filter_admin_keys();
                    }
                });
                
                ui.add_space(5.0);
                
                // Вторая строка - фильтры
                ui.horizontal(|ui| {
                    ui.label("Filter:");
                    let show_deprecated = self.admin_state.show_deprecated_only;
                    if ui.selectable_label(!show_deprecated, "✅ Active").clicked() {
                        self.admin_state.show_deprecated_only = false;
                        self.filter_admin_keys();
                    }
                    if ui.selectable_label(show_deprecated, "❗ Deprecated").clicked() {
                        self.admin_state.show_deprecated_only = true;
                        self.filter_admin_keys();
                    }
                });
            });
        });
        
        ui.add_space(10.0);
        
        // Enhanced bulk actions - лучшие цвета
        let selected_count = self.admin_state.selected_servers.values().filter(|&&v| v).count();
        if selected_count > 0 {
            ui.group(|ui| {
                ui.set_min_width(ui.available_width());
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("📋").size(14.0));
                        ui.label(egui::RichText::new(format!("Selected {} servers", selected_count))
                            .size(14.0)
                            .strong()
                            .color(egui::Color32::LIGHT_BLUE));
                    });
                    
                    ui.add_space(5.0);
                    
                    ui.horizontal(|ui| {
                        if ui.add(egui::Button::new(egui::RichText::new("❗ Deprecate Selected").color(egui::Color32::BLACK))
                            .fill(egui::Color32::from_rgb(255, 200, 0))
                            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(102, 94, 72)))
                            .rounding(egui::Rounding::same(6.0))
                            .min_size(egui::vec2(130.0, 28.0))
                        ).clicked() {
                            self.deprecate_selected_servers(ctx);
                        }
                        
                        if ui.add(egui::Button::new(egui::RichText::new("✅ Restore Selected").color(egui::Color32::WHITE))
                            .fill(egui::Color32::from_rgb(101, 199, 40))
                            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(94, 105, 25)))
                            .rounding(egui::Rounding::same(6.0))
                            .min_size(egui::vec2(120.0, 28.0))
                        ).clicked() {
                            self.restore_selected_servers(ctx);
                        }
                        
                        if ui.add(egui::Button::new(egui::RichText::new("X Clear Selection").color(egui::Color32::WHITE))
                            .fill(egui::Color32::from_rgb(170, 170, 170))
                            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(89, 89, 89)))
                            .rounding(egui::Rounding::same(6.0))
                            .min_size(egui::vec2(110.0, 28.0))
                        ).clicked() {
                            self.admin_state.selected_servers.clear();
                        }
                    });
                });
            });
            ui.add_space(8.0);
        }
        
        // Modern scrollable keys table with better styling
        egui::ScrollArea::vertical()
            .max_height(450.0)
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                if self.admin_state.filtered_keys.is_empty() && !self.admin_state.search_term.is_empty() {
                    ui.vertical_centered(|ui| {
                        ui.add_space(40.0);
                        ui.label(egui::RichText::new("🔍").size(48.0).color(egui::Color32::GRAY));
                        ui.label(egui::RichText::new("No results found")
                            .size(18.0)
                            .color(egui::Color32::GRAY));
                        ui.label(egui::RichText::new(format!("Try adjusting your search: '{}'", self.admin_state.search_term))
                            .size(14.0)
                            .color(egui::Color32::DARK_GRAY));
                    });
                } else {
                    self.render_keys_table(ui, ctx);
                }
            });
    }
    
    fn load_admin_keys(&mut self, ctx: &egui::Context) {
        if self.settings.host.is_empty() || self.settings.flow.is_empty() {
            return;
        }
        
        self.admin_state.current_operation = AdminOperation::LoadingKeys;
        
        let (tx, rx) = mpsc::channel();
        self.admin_receiver = Some(rx);
        
        let host = self.settings.host.clone();
        let flow = self.settings.flow.clone();
        let basic_auth = self.settings.basic_auth.clone();
        let ctx_clone = ctx.clone();
        
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let result = rt.block_on(async {
                fetch_admin_keys(host, flow, basic_auth).await
            });
            
            let _ = tx.send(result);
            ctx_clone.request_repaint();
        });
    }
    
    fn filter_admin_keys(&mut self) {
        let mut filtered = self.admin_state.keys.clone();
        
        // Apply deprecated filter
        if self.admin_state.show_deprecated_only {
            filtered.retain(|key| key.deprecated);
        }
        
        // Apply search filter
        if !self.admin_state.search_term.is_empty() {
            let search_term = self.admin_state.search_term.to_lowercase();
            filtered.retain(|key| {
                key.server.to_lowercase().contains(&search_term) ||
                key.public_key.to_lowercase().contains(&search_term)
            });
        }
        
        self.admin_state.filtered_keys = filtered;
    }
    
    fn render_keys_table(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        if self.admin_state.filtered_keys.is_empty() {
            ui.vertical_centered(|ui| {
                ui.add_space(60.0);
                if self.admin_state.keys.is_empty() {
                    ui.label(egui::RichText::new("🔑").size(48.0).color(egui::Color32::GRAY));
                    ui.label(egui::RichText::new("No SSH keys available")
                        .size(18.0)
                        .color(egui::Color32::GRAY));
                    ui.label(egui::RichText::new("Keys will appear here once loaded from the server")
                        .size(14.0)
                        .color(egui::Color32::DARK_GRAY));
                } else {
                    ui.label(egui::RichText::new("❌").size(48.0).color(egui::Color32::GRAY));
                    ui.label(egui::RichText::new("No keys match current filters")
                        .size(18.0)
                        .color(egui::Color32::GRAY));
                    ui.label(egui::RichText::new("Try adjusting your search or filter settings")
                        .size(14.0)
                        .color(egui::Color32::DARK_GRAY));
                }
            });
            return;
        }
        
        // Group keys by server - clone to avoid borrowing conflicts
        let filtered_keys = self.admin_state.filtered_keys.clone();
        let expanded_servers = self.admin_state.expanded_servers.clone();
        let selected_servers = self.admin_state.selected_servers.clone();
        
        let mut servers: std::collections::BTreeMap<String, Vec<SshKey>> = std::collections::BTreeMap::new();
        for key in &filtered_keys {
            servers.entry(key.server.clone()).or_insert_with(Vec::new).push(key.clone());
        }
        
        // Render each server group
        for (server_name, server_keys) in servers {
            let is_expanded = expanded_servers.get(&server_name).copied().unwrap_or(false);
            let active_count = server_keys.iter().filter(|k| !k.deprecated).count();
            let deprecated_count = server_keys.len() - active_count;
            
            // Modern server header with enhanced styling
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    // Stylized checkbox for server selection
                    let mut selected = selected_servers.get(&server_name).copied().unwrap_or(false);
                    if ui.add(egui::Checkbox::new(&mut selected, "")
                        .indeterminate(false)
                    ).changed() {
                        self.admin_state.selected_servers.insert(server_name.clone(), selected);
                    }
                    
                    // Modern expand/collapse button
                    let expand_icon = if is_expanded { "▼" } else { "▶" };
                    if ui.add(egui::Button::new(expand_icon)
                        .fill(egui::Color32::TRANSPARENT)
                        .stroke(egui::Stroke::NONE)
                        .min_size(egui::vec2(20.0, 20.0))
                    ).clicked() {
                        self.admin_state.expanded_servers.insert(server_name.clone(), !is_expanded);
                    }
                    
                    // Server icon and name
                    ui.label(egui::RichText::new("💻").size(16.0));
                    ui.label(egui::RichText::new(&server_name)
                        .size(15.0)
                        .strong()
                        .color(egui::Color32::WHITE));
                    
                    // Keys count badge - более компактный
                    let (rect, _) = ui.allocate_exact_size(
                        egui::vec2(50.0, 18.0),
                        egui::Sense::hover()
                    );
                    ui.painter().rect_filled(
                        rect,
                        egui::Rounding::same(8.0),
                        egui::Color32::from_rgb(52, 152, 219)
                    );
                    ui.painter().text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        &format!("{} keys", server_keys.len()),
                        egui::FontId::proportional(10.0),
                        egui::Color32::WHITE,
                    );
                    
                    ui.add_space(5.0);
                    
                    // Status indicators - меньшие
                    if deprecated_count > 0 {
                        let (rect, _) = ui.allocate_exact_size(
                            egui::vec2(65.0, 18.0),
                            egui::Sense::hover()
                        );
                        ui.painter().rect_filled(
                            rect,
                            egui::Rounding::same(8.0),
                            egui::Color32::from_rgb(231, 76, 60)
                        );
                        ui.painter().text(
                            rect.center(),
                            egui::Align2::CENTER_CENTER,
                            &format!("{} depr", deprecated_count),
                            egui::FontId::proportional(9.0),
                            egui::Color32::WHITE,
                        );
                    }
                    
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // Stylized action buttons - improved colors
                        if deprecated_count > 0 {
                            if ui.add(egui::Button::new(egui::RichText::new("✅ Restore").color(egui::Color32::WHITE))
                                .fill(egui::Color32::from_rgb(101, 199, 40))
                                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(94, 105, 25)))
                                .rounding(egui::Rounding::same(4.0))
                                .min_size(egui::vec2(70.0, 24.0))
                            ).clicked() {
                                self.restore_server_keys(&server_name, ctx);
                            }
                        }
                        
                        if active_count > 0 {
                            if ui.add(egui::Button::new(egui::RichText::new("❗ Deprecate").color(egui::Color32::BLACK))
                                .fill(egui::Color32::from_rgb(255, 200, 0))
                                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(102, 94, 72)))
                                .rounding(egui::Rounding::same(4.0))
                                .min_size(egui::vec2(85.0, 24.0))
                            ).clicked() {
                                self.deprecate_server_keys(&server_name, ctx);
                            }
                        }
                    });
                });
            });
            
            // Expanded key details
            if is_expanded {
                ui.indent("server_keys", |ui| {
                    for key in &server_keys {
                        ui.group(|ui| {
                            ui.horizontal(|ui| {
                                // Key type badge with modern styling
                                let key_type = self.get_key_type(&key.public_key);
                                let (badge_color, text_color) = match key_type.as_str() {
                                    "RSA" => (egui::Color32::from_rgb(52, 144, 220), egui::Color32::WHITE),
                                    "ED25519" => (egui::Color32::from_rgb(46, 204, 113), egui::Color32::WHITE),
                                    "ECDSA" => (egui::Color32::from_rgb(241, 196, 15), egui::Color32::BLACK),
                                    "DSA" => (egui::Color32::from_rgb(230, 126, 34), egui::Color32::WHITE),
                                    _ => (egui::Color32::GRAY, egui::Color32::WHITE),
                                };
                                
                                // Custom badge rendering - меньшие
                                let (rect, _) = ui.allocate_exact_size(
                                    egui::vec2(40.0, 16.0),
                                    egui::Sense::hover()
                                );
                                ui.painter().rect_filled(
                                    rect,
                                    egui::Rounding::same(3.0),
                                    badge_color
                                );
                                ui.painter().text(
                                    rect.center(),
                                    egui::Align2::CENTER_CENTER,
                                    &key_type,
                                    egui::FontId::proportional(9.0),
                                    text_color,
                                );
                                
                                ui.add_space(5.0);
                                
                                // Status badge with icons - меньшие
                                if key.deprecated {
                                    ui.label(egui::RichText::new("❗ DEPR")
                                        .size(10.0)
                                        .color(egui::Color32::from_rgb(231, 76, 60))
                                        .strong());
                                } else {
                                    ui.label(egui::RichText::new("[OK] ACTIVE")
                                        .size(10.0)
                                        .color(egui::Color32::from_rgb(46, 204, 113))
                                        .strong());
                                }
                                
                                ui.add_space(5.0);
                                
                                // Key preview with monospace font - короче
                                ui.label(egui::RichText::new(self.get_key_preview(&key.public_key))
                                    .font(egui::FontId::monospace(10.0))
                                    .color(egui::Color32::LIGHT_GRAY));
                                
                                let server_name_for_action = server_name.clone();
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    // Modern action buttons - improved colors
                                    if key.deprecated {
                                        if ui.add(egui::Button::new(egui::RichText::new("[R]").color(egui::Color32::WHITE))
                                            .fill(egui::Color32::from_rgb(101, 199, 40))
                                            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(94, 105, 25)))
                                            .rounding(egui::Rounding::same(3.0))
                                            .min_size(egui::vec2(22.0, 18.0))
                                        ).on_hover_text("Restore key").clicked() {
                                            self.restore_key(&server_name_for_action, ctx);
                                        }
                                        if ui.add(egui::Button::new(egui::RichText::new("Del").color(egui::Color32::WHITE))
                                            .fill(egui::Color32::from_rgb(246, 36, 71))
                                            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(129, 18, 17)))
                                            .rounding(egui::Rounding::same(3.0))
                                            .min_size(egui::vec2(26.0, 18.0))
                                        ).on_hover_text("Delete key").clicked() {
                                            self.delete_key(&server_name_for_action, ctx);
                                        }
                                    } else {
                                        if ui.add(egui::Button::new(egui::RichText::new("❗").color(egui::Color32::BLACK))
                                            .fill(egui::Color32::from_rgb(255, 200, 0))
                                            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(102, 94, 72)))
                                            .rounding(egui::Rounding::same(3.0))
                                            .min_size(egui::vec2(22.0, 18.0))
                                        ).on_hover_text("Deprecate key").clicked() {
                                            self.deprecate_key(&server_name_for_action, ctx);
                                        }
                                    }
                                    
                                    if ui.add(egui::Button::new(egui::RichText::new("Copy").color(egui::Color32::WHITE))
                                        .fill(egui::Color32::from_rgb(0, 111, 230))
                                        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(35, 84, 97)))
                                        .rounding(egui::Rounding::same(3.0))
                                        .min_size(egui::vec2(30.0, 18.0))
                                    ).on_hover_text("Copy to clipboard").clicked() {
                                        ui.output_mut(|o| o.copied_text = key.public_key.clone());
                                    }
                                });
                            });
                        });
                    }
                });
            }
            
            ui.add_space(5.0);
        }
    }
    
    fn deprecate_selected_servers(&mut self, ctx: &egui::Context) {
        let selected: Vec<String> = self.admin_state.selected_servers
            .iter()
            .filter_map(|(server, &selected)| if selected { Some(server.clone()) } else { None })
            .collect();
        
        if selected.is_empty() {
            return;
        }
        
        self.admin_state.current_operation = AdminOperation::BulkDeprecating;
        
        let (tx, rx) = mpsc::channel();
        self.operation_receiver = Some(rx);
        
        let host = self.settings.host.clone();
        let flow = self.settings.flow.clone();
        let basic_auth = self.settings.basic_auth.clone();
        let ctx_clone = ctx.clone();
        
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let result = rt.block_on(async {
                bulk_deprecate_servers(host, flow, basic_auth, selected).await
            });
            
            let _ = tx.send(result);
            ctx_clone.request_repaint();
        });
    }
    
    fn restore_selected_servers(&mut self, ctx: &egui::Context) {
        let selected: Vec<String> = self.admin_state.selected_servers
            .iter()
            .filter_map(|(server, &selected)| if selected { Some(server.clone()) } else { None })
            .collect();
        
        if selected.is_empty() {
            return;
        }
        
        self.admin_state.current_operation = AdminOperation::BulkRestoring;
        
        let (tx, rx) = mpsc::channel();
        self.operation_receiver = Some(rx);
        
        let host = self.settings.host.clone();
        let flow = self.settings.flow.clone();
        let basic_auth = self.settings.basic_auth.clone();
        let ctx_clone = ctx.clone();
        
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let result = rt.block_on(async {
                bulk_restore_servers(host, flow, basic_auth, selected).await
            });
            
            let _ = tx.send(result);
            ctx_clone.request_repaint();
        });
    }
    
    fn get_key_type(&self, public_key: &str) -> String {
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
    
    fn get_key_preview(&self, public_key: &str) -> String {
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
    
    fn deprecate_server_keys(&mut self, server: &str, ctx: &egui::Context) {
        self.admin_state.current_operation = AdminOperation::DeprecatingKey;
        
        let (tx, rx) = mpsc::channel();
        self.operation_receiver = Some(rx);
        
        let host = self.settings.host.clone();
        let flow = self.settings.flow.clone();
        let basic_auth = self.settings.basic_auth.clone();
        let server_name = server.to_string();
        let ctx_clone = ctx.clone();
        
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let result = rt.block_on(async {
                deprecate_key_by_server(host, flow, basic_auth, server_name).await
            });
            
            let _ = tx.send(result);
            ctx_clone.request_repaint();
        });
    }
    
    fn restore_server_keys(&mut self, server: &str, ctx: &egui::Context) {
        self.admin_state.current_operation = AdminOperation::RestoringKey;
        
        let (tx, rx) = mpsc::channel();
        self.operation_receiver = Some(rx);
        
        let host = self.settings.host.clone();
        let flow = self.settings.flow.clone();
        let basic_auth = self.settings.basic_auth.clone();
        let server_name = server.to_string();
        let ctx_clone = ctx.clone();
        
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let result = rt.block_on(async {
                restore_key_by_server(host, flow, basic_auth, server_name).await
            });
            
            let _ = tx.send(result);
            ctx_clone.request_repaint();
        });
    }
    
    fn deprecate_key(&mut self, server: &str, ctx: &egui::Context) {
        self.deprecate_server_keys(server, ctx);
    }
    
    fn restore_key(&mut self, server: &str, ctx: &egui::Context) {
        self.restore_server_keys(server, ctx);
    }
    
    fn delete_key(&mut self, server: &str, ctx: &egui::Context) {
        self.admin_state.current_operation = AdminOperation::DeletingKey;
        
        let (tx, rx) = mpsc::channel();
        self.operation_receiver = Some(rx);
        
        let host = self.settings.host.clone();
        let flow = self.settings.flow.clone();
        let basic_auth = self.settings.basic_auth.clone();
        let server_name = server.to_string();
        let ctx_clone = ctx.clone();
        
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let result = rt.block_on(async {
                permanently_delete_key_by_server(host, flow, basic_auth, server_name).await
            });
            
            let _ = tx.send(result);
            ctx_clone.request_repaint();
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
            .with_inner_size([600.0, 800.0])
            .with_min_inner_size([500.0, 650.0])
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
            admin_state: AdminState::default(),
            admin_receiver: None,
            operation_receiver: None,
            current_tab: SettingsTab::Connection,
            is_syncing: false,
            sync_result_receiver: None,
            sync_status: SyncStatus::Unknown,
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

// Admin API functions
async fn bulk_deprecate_servers(host: String, flow: String, basic_auth: String, servers: Vec<String>) -> Result<String, String> {
    if host.is_empty() || flow.is_empty() {
        return Err("Host and flow must be specified".to_string());
    }
    
    let url = format!("{}/{}/bulk-deprecate", host.trim_end_matches('/'), flow);
    info!("Bulk deprecating {} servers at: {}", servers.len(), url);
    
    let client_builder = Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::none());
    
    let client = client_builder.build().map_err(|e| format!("Failed to create HTTP client: {}", e))?;
    
    let mut request = client.post(&url)
        .json(&serde_json::json!({
            "servers": servers
        }));
    
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
        .map_err(|e| format!("Request failed: {}", e))?;
    
    // Check for authentication required
    if response.status().as_u16() == 401 {
        return Err("Authentication required. Please provide valid basic auth credentials.".to_string());
    }
    
    // Check for redirects (usually to login page)
    if response.status().as_u16() >= 300 && response.status().as_u16() < 400 {
        return Err("Server redirects to login page. Authentication may be required.".to_string());
    }
    
    if !response.status().is_success() {
        return Err(format!("Server returned error: {} {}", response.status().as_u16(), response.status().canonical_reason().unwrap_or("Unknown")));
    }
    
    let body = response.text().await
        .map_err(|e| format!("Failed to read response: {}", e))?;
    
    // Parse JSON response to get message
    if let Ok(json_response) = serde_json::from_str::<serde_json::Value>(&body) {
        if let Some(message) = json_response.get("message").and_then(|v| v.as_str()) {
            Ok(message.to_string())
        } else {
            Ok("Successfully deprecated servers".to_string())
        }
    } else {
        Ok("Successfully deprecated servers".to_string())
    }
}

async fn bulk_restore_servers(host: String, flow: String, basic_auth: String, servers: Vec<String>) -> Result<String, String> {
    if host.is_empty() || flow.is_empty() {
        return Err("Host and flow must be specified".to_string());
    }
    
    let url = format!("{}/{}/bulk-restore", host.trim_end_matches('/'), flow);
    info!("Bulk restoring {} servers at: {}", servers.len(), url);
    
    let client_builder = Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::none());
    
    let client = client_builder.build().map_err(|e| format!("Failed to create HTTP client: {}", e))?;
    
    let mut request = client.post(&url)
        .json(&serde_json::json!({
            "servers": servers
        }));
    
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
        .map_err(|e| format!("Request failed: {}", e))?;
    
    // Check for authentication required
    if response.status().as_u16() == 401 {
        return Err("Authentication required. Please provide valid basic auth credentials.".to_string());
    }
    
    // Check for redirects (usually to login page)
    if response.status().as_u16() >= 300 && response.status().as_u16() < 400 {
        return Err("Server redirects to login page. Authentication may be required.".to_string());
    }
    
    if !response.status().is_success() {
        return Err(format!("Server returned error: {} {}", response.status().as_u16(), response.status().canonical_reason().unwrap_or("Unknown")));
    }
    
    let body = response.text().await
        .map_err(|e| format!("Failed to read response: {}", e))?;
    
    // Parse JSON response to get message
    if let Ok(json_response) = serde_json::from_str::<serde_json::Value>(&body) {
        if let Some(message) = json_response.get("message").and_then(|v| v.as_str()) {
            Ok(message.to_string())
        } else {
            Ok("Successfully restored servers".to_string())
        }
    } else {
        Ok("Successfully restored servers".to_string())
    }
}

async fn deprecate_key_by_server(host: String, flow: String, basic_auth: String, server: String) -> Result<String, String> {
    if host.is_empty() || flow.is_empty() {
        return Err("Host and flow must be specified".to_string());
    }
    
    let url = format!("{}/{}/keys/{}", host.trim_end_matches('/'), flow, urlencoding::encode(&server));
    info!("Deprecating key for server '{}' at: {}", server, url);
    
    let client_builder = Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::none());
    
    let client = client_builder.build().map_err(|e| format!("Failed to create HTTP client: {}", e))?;
    
    let mut request = client.delete(&url);
    
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
        .map_err(|e| format!("Request failed: {}", e))?;
    
    // Check for authentication required
    if response.status().as_u16() == 401 {
        return Err("Authentication required. Please provide valid basic auth credentials.".to_string());
    }
    
    // Check for redirects (usually to login page)
    if response.status().as_u16() >= 300 && response.status().as_u16() < 400 {
        return Err("Server redirects to login page. Authentication may be required.".to_string());
    }
    
    if !response.status().is_success() {
        return Err(format!("Server returned error: {} {}", response.status().as_u16(), response.status().canonical_reason().unwrap_or("Unknown")));
    }
    
    let body = response.text().await
        .map_err(|e| format!("Failed to read response: {}", e))?;
    
    // Parse JSON response to get message
    if let Ok(json_response) = serde_json::from_str::<serde_json::Value>(&body) {
        if let Some(message) = json_response.get("message").and_then(|v| v.as_str()) {
            Ok(message.to_string())
        } else {
            Ok(format!("Successfully deprecated key for server '{}'", server))
        }
    } else {
        Ok(format!("Successfully deprecated key for server '{}'", server))
    }
}

async fn restore_key_by_server(host: String, flow: String, basic_auth: String, server: String) -> Result<String, String> {
    if host.is_empty() || flow.is_empty() {
        return Err("Host and flow must be specified".to_string());
    }
    
    let url = format!("{}/{}/keys/{}/restore", host.trim_end_matches('/'), flow, urlencoding::encode(&server));
    info!("Restoring key for server '{}' at: {}", server, url);
    
    let client_builder = Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::none());
    
    let client = client_builder.build().map_err(|e| format!("Failed to create HTTP client: {}", e))?;
    
    let mut request = client.post(&url);
    
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
        .map_err(|e| format!("Request failed: {}", e))?;
    
    // Check for authentication required
    if response.status().as_u16() == 401 {
        return Err("Authentication required. Please provide valid basic auth credentials.".to_string());
    }
    
    // Check for redirects (usually to login page)
    if response.status().as_u16() >= 300 && response.status().as_u16() < 400 {
        return Err("Server redirects to login page. Authentication may be required.".to_string());
    }
    
    if !response.status().is_success() {
        return Err(format!("Server returned error: {} {}", response.status().as_u16(), response.status().canonical_reason().unwrap_or("Unknown")));
    }
    
    let body = response.text().await
        .map_err(|e| format!("Failed to read response: {}", e))?;
    
    // Parse JSON response to get message
    if let Ok(json_response) = serde_json::from_str::<serde_json::Value>(&body) {
        if let Some(message) = json_response.get("message").and_then(|v| v.as_str()) {
            Ok(message.to_string())
        } else {
            Ok(format!("Successfully restored key for server '{}'", server))
        }
    } else {
        Ok(format!("Successfully restored key for server '{}'", server))
    }
}

async fn permanently_delete_key_by_server(host: String, flow: String, basic_auth: String, server: String) -> Result<String, String> {
    if host.is_empty() || flow.is_empty() {
        return Err("Host and flow must be specified".to_string());
    }
    
    let url = format!("{}/{}/keys/{}/delete", host.trim_end_matches('/'), flow, urlencoding::encode(&server));
    info!("Permanently deleting key for server '{}' at: {}", server, url);
    
    let client_builder = Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::none());
    
    let client = client_builder.build().map_err(|e| format!("Failed to create HTTP client: {}", e))?;
    
    let mut request = client.delete(&url);
    
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
        .map_err(|e| format!("Request failed: {}", e))?;
    
    // Check for authentication required
    if response.status().as_u16() == 401 {
        return Err("Authentication required. Please provide valid basic auth credentials.".to_string());
    }
    
    // Check for redirects (usually to login page)
    if response.status().as_u16() >= 300 && response.status().as_u16() < 400 {
        return Err("Server redirects to login page. Authentication may be required.".to_string());
    }
    
    if !response.status().is_success() {
        return Err(format!("Server returned error: {} {}", response.status().as_u16(), response.status().canonical_reason().unwrap_or("Unknown")));
    }
    
    let body = response.text().await
        .map_err(|e| format!("Failed to read response: {}", e))?;
    
    // Parse JSON response to get message
    if let Ok(json_response) = serde_json::from_str::<serde_json::Value>(&body) {
        if let Some(message) = json_response.get("message").and_then(|v| v.as_str()) {
            Ok(message.to_string())
        } else {
            Ok(format!("Successfully deleted key for server '{}'", server))
        }
    } else {
        Ok(format!("Successfully deleted key for server '{}'", server))
    }
}

async fn test_khm_connection(host: String, flow: String, basic_auth: String) -> Result<String, String> {
    if host.is_empty() || flow.is_empty() {
        return Err("Host and flow must be specified".to_string());
    }
    
    let url = format!("{}/{}/keys", host.trim_end_matches('/'), flow);
    info!("Testing connection to: {}", url);
    
    let client_builder = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::none()); // Don't follow redirects
    
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
    
    // Check for authentication required
    if response.status().as_u16() == 401 {
        return Err("Authentication required. Please provide valid basic auth credentials.".to_string());
    }
    
    // Check for redirects (usually to login page)
    if response.status().as_u16() >= 300 && response.status().as_u16() < 400 {
        return Err("Server redirects to login page. Authentication may be required.".to_string());
    }
    
    if !response.status().is_success() {
        return Err(format!("Server returned error: {} {}", response.status().as_u16(), response.status().canonical_reason().unwrap_or("Unknown")));
    }
    
    let body = response.text().await
        .map_err(|e| format!("Failed to read response: {}", e))?;
    
    if body.trim().is_empty() {
        return Err("Server returned empty response".to_string());
    }
    
    // Try to parse as JSON array
    let keys: Result<Vec<serde_json::Value>, _> = serde_json::from_str(&body);
    
    match keys {
        Ok(key_array) => {
            let ssh_key_count = key_array.len();
            Ok(format!("Connection successful! Found {} SSH keys in flow '{}'", ssh_key_count, flow))
        }
        Err(_) => {
            // Check if response looks like HTML (login page)
            if body.trim_start().starts_with("<!DOCTYPE") || body.trim_start().starts_with("<html") {
                return Err("Server returned HTML page instead of JSON. This usually means authentication is required or the endpoint is incorrect.".to_string());
            }
            
            // Fallback: try to parse as plain text (old format)
            let lines: Vec<&str> = body.lines().collect();
            let ssh_key_count = lines.iter()
                .filter(|line| !line.trim().is_empty() && !line.starts_with('#'))
                .count();
            
            if ssh_key_count == 0 && !body.trim().is_empty() {
                return Err("Invalid response format - not JSON array or SSH keys text".to_string());
            }
            
            Ok(format!("Connection successful! Found {} SSH keys in flow '{}'", ssh_key_count, flow))
        }
    }
}

async fn perform_manual_sync(settings: KhmSettings) -> Result<String, String> {
    use crate::Args;
    
    if settings.host.is_empty() || settings.flow.is_empty() {
        return Err("Host and flow must be configured".to_string());
    }
    
    if settings.known_hosts.is_empty() {
        return Err("Known hosts file path must be configured".to_string());
    }
    
    info!("Starting manual sync with host: {}, flow: {}", settings.host, settings.flow);
    
    // Convert KhmSettings to Args for client module
    let args = Args {
        server: false,
        gui: false,
        settings_ui: false,
        in_place: settings.in_place,
        flows: vec!["default".to_string()], // Not used in client mode
        ip: "127.0.0.1".to_string(), // Not used in client mode
        port: 8080, // Not used in client mode
        db_host: "127.0.0.1".to_string(), // Not used in client mode
        db_name: "khm".to_string(), // Not used in client mode
        db_user: None, // Not used in client mode
        db_password: None, // Not used in client mode
        host: Some(settings.host.clone()),
        flow: Some(settings.flow.clone()),
        known_hosts: expand_path(&settings.known_hosts),
        basic_auth: settings.basic_auth.clone(),
    };
    
    // Get keys count before sync
    let keys_before = crate::client::read_known_hosts(&args.known_hosts)
        .unwrap_or_else(|_| Vec::new())
        .len();
    
    // Perform sync
    crate::client::run_client(args.clone()).await
        .map_err(|e| format!("Sync failed: {}", e))?;
    
    // Get keys count after sync
    let keys_after = if args.in_place {
        crate::client::read_known_hosts(&args.known_hosts)
            .unwrap_or_else(|_| Vec::new())
            .len()
    } else {
        keys_before
    };
    
    info!("Manual sync completed: {} keys before, {} keys after", keys_before, keys_after);
    
    let result_message = if args.in_place {
        format!("Sync completed successfully! Known hosts file updated with {} keys (was {})", keys_after, keys_before)
    } else {
        format!("Sync completed successfully! Retrieved {} keys from server", keys_after)
    };
    
    info!("Returning sync result message: '{}'", result_message);
    Ok(result_message)
}
