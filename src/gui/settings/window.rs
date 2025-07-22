use eframe::egui;
use log::info;
use std::sync::mpsc;
use crate::gui::common::{load_settings, KhmSettings};
use crate::gui::admin::{AdminState, AdminOperation, render_statistics, render_search_controls, 
                      render_bulk_actions, render_keys_table, KeyAction, BulkAction};
use crate::gui::api::{SshKey, bulk_deprecate_servers, bulk_restore_servers, 
                     deprecate_key, restore_key, delete_key};

use super::connection::{ConnectionTab, SettingsTab};
use super::ui::{render_connection_tab, add_log_entry};

pub struct SettingsWindow {
    settings: KhmSettings,
    auto_sync_interval_str: String,
    current_tab: SettingsTab,
    connection_tab: ConnectionTab,
    admin_state: AdminState,
    admin_receiver: Option<mpsc::Receiver<Result<Vec<SshKey>, String>>>,
    operation_receiver: Option<mpsc::Receiver<Result<String, String>>>,
    operation_log: Vec<String>,
}

impl SettingsWindow {
    pub fn new() -> Self {
        let settings = load_settings();
        let auto_sync_interval_str = settings.auto_sync_interval_minutes.to_string();
        
        Self {
            settings,
            auto_sync_interval_str,
            current_tab: SettingsTab::Connection,
            connection_tab: ConnectionTab::default(),
            admin_state: AdminState::default(),
            admin_receiver: None,
            operation_receiver: None,
            operation_log: Vec::new(),
        }
    }
}

impl eframe::App for SettingsWindow {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Check for admin operation results
        self.check_admin_results(ctx);
        
        // Apply enhanced modern dark theme
        apply_modern_theme(ctx);
        
        // Bottom panel for Activity Log (fixed at bottom)
        egui::TopBottomPanel::bottom("activity_log_panel")
            .resizable(false)
            .min_height(140.0)
            .max_height(140.0)
            .frame(egui::Frame::none()
                .fill(egui::Color32::from_gray(12))
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_gray(60)))
            )
            .show(ctx, |ui| {
                render_bottom_activity_log(ui, &mut self.operation_log);
            });
        
        egui::CentralPanel::default()
            .frame(egui::Frame::none()
                .fill(egui::Color32::from_gray(18))
                .inner_margin(egui::Margin::same(20.0))
            )
            .show(ctx, |ui| {
                // Modern header with gradient-like styling
                let header_frame = egui::Frame::none()
                    .fill(ui.visuals().panel_fill)
                    .rounding(egui::Rounding::same(8.0))
                    .inner_margin(egui::Margin::same(12.0))
                    .stroke(egui::Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color));
                
                header_frame.show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.add_space(4.0);
                        ui.label("🔑");
                        ui.heading(egui::RichText::new("KHM Settings").size(20.0).strong());
                        ui.label(egui::RichText::new(
                            "(Known Hosts Manager for SSH key management and synchronization)"
                        ).size(11.0).weak().italics());
                        
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            // Version from Cargo.toml
                            let version = env!("CARGO_PKG_VERSION");
                            if ui.small_button(format!("v{}", version))
                                .on_hover_text(format!(
                                    "{}\n{}\nRepository: {}\nLicense: {}", 
                                    env!("CARGO_PKG_DESCRIPTION"),
                                    env!("CARGO_PKG_AUTHORS"),
                                    env!("CARGO_PKG_REPOSITORY"),
                                    "WTFPL"
                                ))
                                .clicked() 
                            {
                                // Open repository URL
                                if let Err(_) = std::process::Command::new("open")
                                    .arg(env!("CARGO_PKG_REPOSITORY"))
                                    .spawn() 
                                {
                                    // Fallback for non-macOS systems
                                    let _ = std::process::Command::new("xdg-open")
                                        .arg(env!("CARGO_PKG_REPOSITORY"))
                                        .spawn();
                                }
                            }
                        });
                    });
                });
                
                ui.add_space(12.0);
                
                // Modern tab selector with card styling
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    
                    // Connection/Settings Tab
                    let connection_selected = matches!(self.current_tab, SettingsTab::Connection);
                    let connection_button = egui::Button::new(
                        egui::RichText::new("🌐 Connection").size(13.0)
                    )
                    .fill(if connection_selected { 
                        egui::Color32::from_rgb(0, 120, 212)
                    } else {
                        ui.visuals().widgets.inactive.bg_fill
                    })
                    .stroke(if connection_selected {
                        egui::Stroke::new(1.0, egui::Color32::from_rgb(0, 120, 212))
                    } else {
                        egui::Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color)
                    })
                    .rounding(6.0)
                    .min_size(egui::vec2(110.0, 32.0));
                    
                    if ui.add(connection_button).clicked() {
                        self.current_tab = SettingsTab::Connection;
                    }
                    
                    // Admin Tab
                    let admin_selected = matches!(self.current_tab, SettingsTab::Admin);
                    let admin_button = egui::Button::new(
                        egui::RichText::new("🔧 Admin Panel").size(13.0)
                    )
                    .fill(if admin_selected { 
                        egui::Color32::from_rgb(120, 80, 0)
                    } else {
                        ui.visuals().widgets.inactive.bg_fill
                    })
                    .stroke(if admin_selected {
                        egui::Stroke::new(1.0, egui::Color32::from_rgb(120, 80, 0))
                    } else {
                        egui::Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color)
                    })
                    .rounding(6.0)
                    .min_size(egui::vec2(110.0, 32.0));
                    
                    if ui.add(admin_button).clicked() {
                        self.current_tab = SettingsTab::Admin;
                    }
                });
                
                ui.add_space(16.0);
                
                // Content area with proper spacing
                match self.current_tab {
                    SettingsTab::Connection => {
                        render_connection_tab(
                            ui, 
                            ctx, 
                            &mut self.settings, 
                            &mut self.auto_sync_interval_str,
                            &mut self.connection_tab,
                            &mut self.operation_log
                        );
                    }
                    SettingsTab::Admin => {
                        self.render_admin_tab(ui, ctx);
                    }
                }
            });
    }
}

impl SettingsWindow {
    fn check_admin_results(&mut self, ctx: &egui::Context) {
        // Check for admin keys loading result
        if let Some(receiver) = &self.admin_receiver {
            if let Ok(result) = receiver.try_recv() {
                self.admin_state.handle_keys_loaded(result);
                self.admin_receiver = None;
                ctx.request_repaint();
            }
        }
        
        // Check for operation results
        if let Some(receiver) = &self.operation_receiver {
            if let Ok(result) = receiver.try_recv() {
                match result {
                    Ok(message) => {
                        info!("Operation completed: {}", message);
                        add_log_entry(&mut self.operation_log, format!("✅ {}", message));
                        // Reload keys after operation
                        self.load_admin_keys(ctx);
                    }
                    Err(error) => {
                        add_log_entry(&mut self.operation_log, format!("❌ Operation failed: {}", error));
                    }
                }
                self.admin_state.current_operation = AdminOperation::None;
                self.operation_receiver = None;
                ctx.request_repaint();
            }
        }
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
        
        // Statistics section
        render_statistics(ui, &self.admin_state);
        ui.add_space(10.0);
        
        // Search and filters
        render_search_controls(ui, &mut self.admin_state);
        ui.add_space(10.0);
        
        // Bulk actions
        let bulk_action = render_bulk_actions(ui, &mut self.admin_state);
        self.handle_bulk_action(bulk_action, ctx);
        
        if self.admin_state.selected_servers.values().any(|&v| v) {
            ui.add_space(8.0);
        }
        
        // Keys table
        egui::ScrollArea::vertical()
            .max_height(450.0)
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                let key_action = render_keys_table(ui, &mut self.admin_state);
                self.handle_key_action(key_action, ctx);
            });
    }
    
    fn load_admin_keys(&mut self, ctx: &egui::Context) {
        if let Some(receiver) = self.admin_state.load_keys(&self.settings, ctx) {
            self.admin_receiver = Some(receiver);
        }
    }
    
    fn handle_bulk_action(&mut self, action: BulkAction, ctx: &egui::Context) {
        match action {
            BulkAction::DeprecateSelected => {
                let selected = self.admin_state.get_selected_servers();
                if !selected.is_empty() {
                    self.start_bulk_deprecate(selected, ctx);
                }
            }
            BulkAction::RestoreSelected => {
                let selected = self.admin_state.get_selected_servers();
                if !selected.is_empty() {
                    self.start_bulk_restore(selected, ctx);
                }
            }
            BulkAction::ClearSelection => {
                // Selection already cleared in UI
            }
            BulkAction::None => {}
        }
    }
    
    fn handle_key_action(&mut self, action: KeyAction, ctx: &egui::Context) {
        match action {
            KeyAction::DeprecateKey(server) | KeyAction::DeprecateServer(server) => {
                self.start_deprecate_key(&server, ctx);
            }
            KeyAction::RestoreKey(server) | KeyAction::RestoreServer(server) => {
                self.start_restore_key(&server, ctx);
            }
            KeyAction::DeleteKey(server) => {
                self.start_delete_key(&server, ctx);
            }
            KeyAction::None => {}
        }
    }
    
    fn start_bulk_deprecate(&mut self, servers: Vec<String>, ctx: &egui::Context) {
        self.admin_state.current_operation = AdminOperation::BulkDeprecating;
        add_log_entry(&mut self.operation_log, format!("Deprecating {} servers...", servers.len()));
        
        let (tx, rx) = mpsc::channel();
        self.operation_receiver = Some(rx);
        
        let host = self.settings.host.clone();
        let flow = self.settings.flow.clone();
        let basic_auth = self.settings.basic_auth.clone();
        let ctx_clone = ctx.clone();
        
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let result = rt.block_on(async {
                bulk_deprecate_servers(host, flow, basic_auth, servers).await
            });
            
            let _ = tx.send(result);
            ctx_clone.request_repaint();
        });
    }
    
    fn start_bulk_restore(&mut self, servers: Vec<String>, ctx: &egui::Context) {
        self.admin_state.current_operation = AdminOperation::BulkRestoring;
        add_log_entry(&mut self.operation_log, format!("Restoring {} servers...", servers.len()));
        
        let (tx, rx) = mpsc::channel();
        self.operation_receiver = Some(rx);
        
        let host = self.settings.host.clone();
        let flow = self.settings.flow.clone();
        let basic_auth = self.settings.basic_auth.clone();
        let ctx_clone = ctx.clone();
        
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let result = rt.block_on(async {
                bulk_restore_servers(host, flow, basic_auth, servers).await
            });
            
            let _ = tx.send(result);
            ctx_clone.request_repaint();
        });
    }
    
    fn start_deprecate_key(&mut self, server: &str, ctx: &egui::Context) {
        self.admin_state.current_operation = AdminOperation::DeprecatingKey;
        add_log_entry(&mut self.operation_log, format!("Deprecating key for server: {}", server));
        
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
                deprecate_key(host, flow, basic_auth, server_name).await
            });
            
            let _ = tx.send(result);
            ctx_clone.request_repaint();
        });
    }
    
    fn start_restore_key(&mut self, server: &str, ctx: &egui::Context) {
        self.admin_state.current_operation = AdminOperation::RestoringKey;
        add_log_entry(&mut self.operation_log, format!("Restoring key for server: {}", server));
        
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
                restore_key(host, flow, basic_auth, server_name).await
            });
            
            let _ = tx.send(result);
            ctx_clone.request_repaint();
        });
    }
    
    fn start_delete_key(&mut self, server: &str, ctx: &egui::Context) {
        self.admin_state.current_operation = AdminOperation::DeletingKey;
        add_log_entry(&mut self.operation_log, format!("Deleting key for server: {}", server));
        
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
                delete_key(host, flow, basic_auth, server_name).await
            });
            
            let _ = tx.send(result);
            ctx_clone.request_repaint();
        });
    }
}

/// Apply modern dark theme for the settings window with enhanced styling
fn apply_modern_theme(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    
    // Modern color palette
    visuals.window_fill = egui::Color32::from_gray(18);  // Darker background
    visuals.panel_fill = egui::Color32::from_gray(24);   // Panel background
    visuals.faint_bg_color = egui::Color32::from_gray(32); // Card background
    visuals.extreme_bg_color = egui::Color32::from_gray(12); // Darkest areas
    
    // Enhanced widget styling
    visuals.button_frame = true;
    visuals.collapsing_header_frame = true;
    visuals.indent_has_left_vline = true;
    visuals.striped = true;
    
    // Modern rounded corners
    let rounding = egui::Rounding::same(8.0);
    visuals.menu_rounding = rounding;
    visuals.window_rounding = egui::Rounding::same(16.0);
    visuals.widgets.noninteractive.rounding = rounding;
    visuals.widgets.inactive.rounding = rounding;
    visuals.widgets.hovered.rounding = rounding;
    visuals.widgets.active.rounding = rounding;
    
    // Better widget colors
    visuals.widgets.noninteractive.bg_fill = egui::Color32::from_gray(40);
    visuals.widgets.inactive.bg_fill = egui::Color32::from_gray(45);
    visuals.widgets.hovered.bg_fill = egui::Color32::from_gray(55);
    visuals.widgets.active.bg_fill = egui::Color32::from_gray(60);
    
    // Subtle borders
    let border_color = egui::Color32::from_gray(60);
    visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, border_color);
    visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, border_color);
    visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.5, egui::Color32::from_gray(80));
    visuals.widgets.active.bg_stroke = egui::Stroke::new(1.5, egui::Color32::from_gray(100));
    
    ctx.set_visuals(visuals);
}

/// Render bottom activity log panel
fn render_bottom_activity_log(ui: &mut egui::Ui, operation_log: &mut Vec<String>) {
    ui.add_space(18.0); // Larger top padding
    
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        ui.label("📋");
        ui.label(egui::RichText::new("Activity Log").size(13.0).strong());
        
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
            ui.add_space(8.0);
            if ui.small_button("🗑 Clear").clicked() {
                operation_log.clear();
            }
        });
    });
    
    ui.add_space(8.0);
    
    // Add horizontal margin for the text area
    ui.horizontal(|ui| {
        ui.add_space(8.0); // Left margin
        
        // Show last 5 log entries in multiline text
        let log_text = if operation_log.is_empty() {
            "No recent activity".to_string()
        } else {
            let start_idx = if operation_log.len() > 5 {
                operation_log.len() - 5
            } else {
                0
            };
            operation_log[start_idx..].join("\n")
        };
        
        ui.add_sized(
            [ui.available_width() - 8.0, 80.0], // Account for right margin
            egui::TextEdit::multiline(&mut log_text.clone())
                .font(egui::FontId::new(11.0, egui::FontFamily::Monospace))
                .interactive(false)
        );
        
        ui.add_space(8.0); // Right margin
    });
}

/// Create window icon for settings window
pub fn create_window_icon() -> egui::IconData {
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

/// Run the settings window application with modern horizontal styling
pub fn run_settings_window() {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("KHM Settings")
            .with_inner_size([900.0, 905.0])  // Decreased height by another 15px
            .with_min_inner_size([900.0, 905.0])  // Fixed size
            .with_max_inner_size([900.0, 905.0]) // Same as min - fixed size
            .with_resizable(false) // Disable resizing since window is fixed size
            .with_icon(create_window_icon())
            .with_decorations(true)
            .with_transparent(false),
        centered: true,
        ..Default::default()
    };
    
    let _ = eframe::run_native(
        "KHM Settings",
        options,
        Box::new(|_cc| Ok(Box::new(SettingsWindow::new()))),
    );
}
