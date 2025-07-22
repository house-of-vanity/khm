use eframe::egui;
use crate::gui::common::{KhmSettings, get_config_path};
use super::connection::{ConnectionTab, ConnectionStatus, SyncStatus, save_settings_validated};

/// Render connection settings tab
pub fn render_connection_tab(
    ui: &mut egui::Ui, 
    ctx: &egui::Context,
    settings: &mut KhmSettings,
    auto_sync_interval_str: &mut String,
    connection_tab: &mut ConnectionTab,
    operation_log: &mut Vec<String>
) {
    // Check for connection test and sync results
    connection_tab.check_results(ctx, settings);
    
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
                    render_connection_section(ui, settings, connection_tab);
                    ui.add_space(10.0);
                    
                    // Local settings section
                    render_local_settings_section(ui, settings);
                    ui.add_space(15.0);
                    
                    // Auto-sync section
                    render_auto_sync_section(ui, settings, auto_sync_interval_str);
                    ui.add_space(10.0);
                    
                    // Configuration file location
                    render_config_location_section(ui);
                });
        },
    );
    
    // Bottom area with buttons and log
    render_bottom_area(ui, ctx, settings, connection_tab, operation_log);
}

fn render_connection_section(ui: &mut egui::Ui, settings: &mut KhmSettings, connection_tab: &ConnectionTab) {
    ui.group(|ui| {
        ui.set_min_width(ui.available_width());
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("🌐 Connection").size(16.0).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Display connection status with details
                    match &connection_tab.connection_status {
                        ConnectionStatus::Connected { keys_count, flow } => {
                            let status_text = if flow.is_empty() {
                                format!("Connected ({} keys)", keys_count)
                            } else {
                                format!("Connected to '{}' ({} keys)", flow, keys_count)
                            };
                            ui.add_enabled(false, egui::Checkbox::new(&mut true, status_text));
                        }
                        ConnectionStatus::Error(error_msg) => {
                            ui.label(egui::RichText::new("❌ Error").color(egui::Color32::RED))
                                .on_hover_text(error_msg);
                        }
                        ConnectionStatus::Unknown => {
                            ui.add_enabled(false, egui::Checkbox::new(&mut false, "Not connected"));
                        }
                    }
                    
                    if connection_tab.is_testing_connection {
                        ui.spinner();
                        ui.label(egui::RichText::new("Testing...").italics());
                    }
                });
            });
            
            // Display sync status if available
            match &connection_tab.sync_status {
                SyncStatus::Success { keys_count } => {
                    ui.horizontal(|ui| {
                        ui.label("🔄 Last sync:");
                        ui.label(egui::RichText::new(format!("{} keys synced", keys_count))
                            .color(egui::Color32::GREEN));
                    });
                }
                SyncStatus::Error(error_msg) => {
                    ui.horizontal(|ui| {
                        ui.label("🔄 Last sync:");
                        ui.label(egui::RichText::new("Failed")
                            .color(egui::Color32::RED))
                            .on_hover_text(error_msg);
                    });
                }
                SyncStatus::Unknown => {}
            }
            
            ui.add_space(5.0);
            
            egui::Grid::new("connection_grid")
                .num_columns(2)
                .min_col_width(120.0)
                .spacing([10.0, 8.0])
                .show(ui, |ui| {
                    ui.label("Host URL:");
                    ui.add_sized(
                        [ui.available_width(), 20.0],
                        egui::TextEdit::singleline(&mut settings.host)
                            .hint_text("https://your-khm-server.com")
                    );
                    ui.end_row();
                    
                    ui.label("Flow Name:");
                    ui.add_sized(
                        [ui.available_width(), 20.0],
                        egui::TextEdit::singleline(&mut settings.flow)
                            .hint_text("production, staging, etc.")
                    );
                    ui.end_row();
                    
                    ui.label("Basic Auth:");
                    ui.add_sized(
                        [ui.available_width(), 20.0],
                        egui::TextEdit::singleline(&mut settings.basic_auth)
                            .hint_text("username:password (optional)")
                            .password(true)
                    );
                    ui.end_row();
                });
        });
    });
}

fn render_local_settings_section(ui: &mut egui::Ui, settings: &mut KhmSettings) {
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
                        egui::TextEdit::singleline(&mut settings.known_hosts)
                            .hint_text("~/.ssh/known_hosts")
                    );
                    ui.end_row();
                });
            
            ui.add_space(8.0);
            ui.checkbox(&mut settings.in_place, "✏ Update known_hosts file in-place after sync");
        });
    });
}

fn render_auto_sync_section(ui: &mut egui::Ui, settings: &mut KhmSettings, auto_sync_interval_str: &mut String) {
    ui.group(|ui| {
        ui.set_min_width(ui.available_width());
        ui.vertical(|ui| {
            ui.label(egui::RichText::new("🔄 Auto Sync").size(16.0).strong());
            ui.add_space(8.0);
            
            let is_auto_sync_enabled = !settings.host.is_empty() 
                && !settings.flow.is_empty() 
                && settings.in_place;
            
            ui.horizontal(|ui| {
                ui.label("Interval (minutes):");
                ui.add_sized(
                    [80.0, 20.0],
                    egui::TextEdit::singleline(auto_sync_interval_str)
                );
                
                if let Ok(value) = auto_sync_interval_str.parse::<u32>() {
                    if value > 0 {
                        settings.auto_sync_interval_minutes = value;
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
}

fn render_config_location_section(ui: &mut egui::Ui) {
    ui.group(|ui| {
        ui.set_min_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.label("🗁 Config file:");
            let config_path = get_config_path();
            ui.add_sized(
                [ui.available_width(), 20.0],
                egui::TextEdit::singleline(&mut config_path.display().to_string())
                    .interactive(false)
            );
        });
    });
}

fn render_bottom_area(
    ui: &mut egui::Ui, 
    ctx: &egui::Context,
    settings: &KhmSettings,
    connection_tab: &mut ConnectionTab,
    operation_log: &mut Vec<String>
) {
    let button_area_height = 120.0;
    
    ui.allocate_ui_with_layout(
        [ui.available_width(), button_area_height].into(),
        egui::Layout::bottom_up(egui::Align::Min),
        |ui| {
            // Operation log area
            ui.group(|ui| {
                ui.set_min_width(ui.available_width());
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new("📄 Operation Log").size(14.0).strong());
                    ui.add_space(5.0);
                    
                    let log_text = operation_log.join("\n");
                    ui.add_sized(
                        [ui.available_width(), 60.0],
                        egui::TextEdit::multiline(&mut log_text.clone())
                            .font(egui::FontId::monospace(10.0))
                            .interactive(false)
                    );
                });
            });
            
            ui.add_space(8.0);
            
            // Show validation hints
            let save_enabled = !settings.host.is_empty() && !settings.flow.is_empty();
            if !save_enabled {
                ui.label(egui::RichText::new("❗ Please fill in Host URL and Flow Name to save settings")
                    .color(egui::Color32::YELLOW)
                    .italics());
            }
            
            ui.add_space(5.0);
            
            // Action buttons
            render_action_buttons(ui, ctx, settings, connection_tab, save_enabled, operation_log);
        },
    );
}

fn render_action_buttons(
    ui: &mut egui::Ui, 
    ctx: &egui::Context,
    settings: &KhmSettings,
    connection_tab: &mut ConnectionTab,
    save_enabled: bool,
    operation_log: &mut Vec<String>
) {
    ui.horizontal(|ui| {
        if ui.add_enabled(
            save_enabled,
            egui::Button::new("💾 Save Settings")
                .min_size(egui::vec2(120.0, 32.0))
        ).clicked() {
            match save_settings_validated(settings) {
                Ok(()) => {
                    add_log_entry(operation_log, "✅ Settings saved successfully".to_string());
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                Err(e) => {
                    add_log_entry(operation_log, format!("❌ Failed to save settings: {}", e));
                }
            }
        }
        
        if ui.add(
            egui::Button::new("✖ Cancel")
                .min_size(egui::vec2(80.0, 32.0))
        ).clicked() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let can_test = !settings.host.is_empty() && !settings.flow.is_empty() && !connection_tab.is_testing_connection;
            let can_sync = !settings.host.is_empty() && !settings.flow.is_empty() && !connection_tab.is_syncing;
            
            if ui.add_enabled(
                can_test,
                egui::Button::new(
                    if connection_tab.is_testing_connection {
                        "▶ Testing..."
                    } else {
                        "🔍 Test Connection"
                    }
                ).min_size(egui::vec2(120.0, 32.0))
            ).clicked() {
                add_log_entry(operation_log, "🔍 Starting connection test...".to_string());
                connection_tab.start_test(settings, ctx);
            }
            
            if ui.add_enabled(
                can_sync,
                egui::Button::new(
                    if connection_tab.is_syncing {
                        "🔄 Syncing..."
                    } else {
                        "🔄 Sync Now"
                    }
                ).min_size(egui::vec2(100.0, 32.0))
            ).clicked() {
                add_log_entry(operation_log, "🔄 Starting manual sync...".to_string());
                connection_tab.start_sync(settings, ctx);
            }
        });
    });
}

/// Add entry to operation log with timestamp
pub fn add_log_entry(operation_log: &mut Vec<String>, message: String) {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap();
    let secs = now.as_secs();
    let millis = now.subsec_millis();
    
    // Format as HH:MM:SS.mmm
    let hours = (secs / 3600) % 24;
    let minutes = (secs / 60) % 60;
    let seconds = secs % 60;
    let timestamp = format!("{:02}:{:02}:{:02}.{:03}", hours, minutes, seconds, millis);
    
    let log_entry = format!("{} {}", timestamp, message);
    
    operation_log.push(log_entry);
    
    // Keep only last 20 entries to prevent memory growth
    if operation_log.len() > 20 {
        operation_log.remove(0);
    }
}
