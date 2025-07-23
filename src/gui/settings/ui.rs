use super::connection::{save_settings_validated, ConnectionStatus, ConnectionTab, SyncStatus};
use crate::gui::common::{get_config_path, KhmSettings};
use eframe::egui;

/// Render connection settings tab with modern horizontal UI design
pub fn render_connection_tab(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    settings: &mut KhmSettings,
    auto_sync_interval_str: &mut String,
    connection_tab: &mut ConnectionTab,
    operation_log: &mut Vec<String>,
) {
    // Check for connection test and sync results
    connection_tab.check_results(ctx, settings, operation_log);

    // Use scrollable area for the entire content
    egui::ScrollArea::vertical()
        .auto_shrink([false; 2])
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(6.0, 8.0);
            ui.spacing_mut().button_padding = egui::vec2(12.0, 6.0);
            ui.spacing_mut().indent = 16.0;

            // Connection Status Card at top (full width)
            render_connection_status_card(ui, connection_tab);

            // Main configuration area - horizontal layout
            ui.horizontal_top(|ui| {
                let available_width = ui.available_width();
                let left_panel_width = available_width * 0.6;
                let right_panel_width = available_width * 0.38;

                // Left panel - Connection and Local config
                ui.allocate_ui_with_layout(
                    [left_panel_width, ui.available_height()].into(),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        // Connection Configuration Card
                        render_connection_config_card(ui, settings);

                        // Local Configuration Card
                        render_local_config_card(ui, settings);
                    },
                );

                ui.add_space(8.0);

                // Right panel - Auto-sync and System info
                ui.allocate_ui_with_layout(
                    [right_panel_width, ui.available_height()].into(),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        // Auto-sync Configuration Card
                        render_auto_sync_card(ui, settings, auto_sync_interval_str);

                        // System Information Card
                        render_system_info_card(ui);
                    },
                );
            });

            ui.add_space(12.0);

            // Action buttons at bottom
            render_action_section(ui, ctx, settings, connection_tab, operation_log);
        });
}

/// Connection status card with modern visual design
fn render_connection_status_card(ui: &mut egui::Ui, connection_tab: &ConnectionTab) {
    let frame = egui::Frame::group(ui.style())
        .fill(ui.visuals().faint_bg_color)
        .stroke(egui::Stroke::new(
            1.0,
            ui.visuals().widgets.noninteractive.bg_stroke.color,
        ))
        .rounding(6.0)
        .inner_margin(egui::Margin::same(12.0));

    frame.show(ui, |ui| {
        // Header with status indicator
        ui.horizontal(|ui| {
            let (status_icon, status_text, status_color) = match &connection_tab.connection_status {
                ConnectionStatus::Connected { keys_count, flow } => {
                    let text = if flow.is_empty() {
                        format!("Connected • {} keys", keys_count)
                    } else {
                        format!("Connected to '{}' • {} keys", flow, keys_count)
                    };
                    ("✅", text, egui::Color32::GREEN)
                }
                ConnectionStatus::Error(error_msg) => (
                    "❌",
                    format!("Connection Error: {}", error_msg),
                    egui::Color32::RED,
                ),
                ConnectionStatus::Unknown => {
                    ("⚫", "Not Connected".to_string(), ui.visuals().text_color())
                }
            };

            ui.label(egui::RichText::new(status_icon).size(14.0));
            ui.label(egui::RichText::new("Connection Status").size(14.0).strong());

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if connection_tab.is_testing_connection {
                    ui.spinner();
                    ui.label(
                        egui::RichText::new("Testing...")
                            .italics()
                            .color(ui.visuals().weak_text_color()),
                    );
                } else {
                    ui.label(
                        egui::RichText::new(&status_text)
                            .size(13.0)
                            .color(status_color),
                    );
                }
            });
        });

        // Sync status - always visible
        ui.add_space(6.0);
        ui.separator();
        ui.add_space(6.0);

        ui.horizontal(|ui| {
            ui.label("🔄");
            ui.label("Last Sync:");

            match &connection_tab.sync_status {
                SyncStatus::Success { keys_count } => {
                    ui.label(
                        egui::RichText::new(format!("✅ {} keys synced", keys_count))
                            .size(13.0)
                            .color(egui::Color32::GREEN),
                    );
                }
                SyncStatus::Error(error_msg) => {
                    ui.label(
                        egui::RichText::new("❌ Failed")
                            .size(13.0)
                            .color(egui::Color32::RED),
                    )
                    .on_hover_text(error_msg);
                }
                SyncStatus::Unknown => {
                    ui.label(
                        egui::RichText::new("No sync performed yet")
                            .size(13.0)
                            .color(ui.visuals().weak_text_color()),
                    );
                }
            }
        });
    });

    ui.add_space(8.0);
}

/// Connection configuration card with input fields
fn render_connection_config_card(ui: &mut egui::Ui, settings: &mut KhmSettings) {
    let frame = egui::Frame::group(ui.style())
        .fill(ui.visuals().faint_bg_color)
        .stroke(egui::Stroke::new(
            1.0,
            ui.visuals().widgets.noninteractive.bg_stroke.color,
        ))
        .rounding(6.0)
        .inner_margin(egui::Margin::same(12.0));

    frame.show(ui, |ui| {
        // Header
        ui.horizontal(|ui| {
            ui.label("🌐");
            ui.label(
                egui::RichText::new("Server Configuration")
                    .size(14.0)
                    .strong(),
            );
        });

        ui.add_space(8.0);

        // Input fields with better spacing
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 8.0;

            // Host URL
            ui.vertical(|ui| {
                ui.label(egui::RichText::new("Host URL").size(13.0).strong());
                ui.add_space(3.0);
                ui.add_sized(
                    [ui.available_width(), 28.0], // Smaller height for better centering
                    egui::TextEdit::singleline(&mut settings.host)
                        .hint_text("https://your-khm-server.com")
                        .font(egui::FontId::new(14.0, egui::FontFamily::Monospace))
                        .margin(egui::Margin::symmetric(8.0, 6.0)), // Better vertical centering
                );
            });

            // Flow Name
            ui.vertical(|ui| {
                ui.label(egui::RichText::new("Flow Name").size(13.0).strong());
                ui.add_space(3.0);
                ui.add_sized(
                    [ui.available_width(), 28.0],
                    egui::TextEdit::singleline(&mut settings.flow)
                        .hint_text("production, staging, development")
                        .font(egui::FontId::new(14.0, egui::FontFamily::Proportional))
                        .margin(egui::Margin::symmetric(8.0, 6.0)),
                );
            });

            // Basic Auth (optional)
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("Basic Authentication")
                            .size(13.0)
                            .strong(),
                    );
                    ui.label(
                        egui::RichText::new("(optional)")
                            .size(12.0)
                            .weak()
                            .italics(),
                    );
                });
                ui.add_space(3.0);
                ui.add_sized(
                    [ui.available_width(), 28.0],
                    egui::TextEdit::singleline(&mut settings.basic_auth)
                        .hint_text("username:password")
                        .password(true)
                        .font(egui::FontId::new(14.0, egui::FontFamily::Monospace))
                        .margin(egui::Margin::symmetric(8.0, 6.0)),
                );
            });
        });
    });

    ui.add_space(8.0);
}

/// Local configuration card
fn render_local_config_card(ui: &mut egui::Ui, settings: &mut KhmSettings) {
    let frame = egui::Frame::group(ui.style())
        .fill(ui.visuals().faint_bg_color)
        .stroke(egui::Stroke::new(
            1.0,
            ui.visuals().widgets.noninteractive.bg_stroke.color,
        ))
        .rounding(6.0)
        .inner_margin(egui::Margin::same(12.0));

    frame.show(ui, |ui| {
        // Header
        ui.horizontal(|ui| {
            ui.label("📁");
            ui.label(
                egui::RichText::new("Local Configuration")
                    .size(14.0)
                    .strong(),
            );
        });

        ui.add_space(8.0);

        // Known hosts file
        ui.vertical(|ui| {
            ui.label(
                egui::RichText::new("Known Hosts File Path")
                    .size(13.0)
                    .strong(),
            );
            ui.add_space(3.0);
            ui.add_sized(
                [ui.available_width(), 28.0],
                egui::TextEdit::singleline(&mut settings.known_hosts)
                    .hint_text("~/.ssh/known_hosts")
                    .font(egui::FontId::new(14.0, egui::FontFamily::Monospace))
                    .margin(egui::Margin::symmetric(8.0, 6.0)),
            );

            ui.add_space(8.0);

            // In-place update option with better styling
            ui.horizontal(|ui| {
                ui.checkbox(&mut settings.in_place, "");
                ui.vertical(|ui| {
                    ui.label(
                        egui::RichText::new("Update file in-place after sync")
                            .size(13.0)
                            .strong(),
                    );
                    ui.label(
                        egui::RichText::new(
                            "Automatically modify the known_hosts file when synchronizing",
                        )
                        .size(12.0)
                        .weak()
                        .italics(),
                    );
                });
            });
        });
    });

    ui.add_space(8.0);
}

/// Auto-sync configuration card
fn render_auto_sync_card(
    ui: &mut egui::Ui,
    settings: &mut KhmSettings,
    auto_sync_interval_str: &mut String,
) {
    let frame = egui::Frame::group(ui.style())
        .fill(ui.visuals().faint_bg_color)
        .stroke(egui::Stroke::new(
            1.0,
            ui.visuals().widgets.noninteractive.bg_stroke.color,
        ))
        .rounding(6.0)
        .inner_margin(egui::Margin::same(12.0));

    frame.show(ui, |ui| {
        let is_auto_sync_enabled =
            !settings.host.is_empty() && !settings.flow.is_empty() && settings.in_place;

        // Header with status
        ui.horizontal(|ui| {
            ui.label("🔄");
            ui.label(egui::RichText::new("Auto Sync").size(14.0).strong());

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let (status_text, status_color) = if is_auto_sync_enabled {
                    ("✅ Active", egui::Color32::GREEN)
                } else {
                    ("❌ Inactive", egui::Color32::from_gray(128))
                };

                ui.label(
                    egui::RichText::new(status_text)
                        .size(12.0)
                        .color(status_color),
                );
            });
        });

        ui.add_space(8.0);

        // Interval setting
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Interval").size(13.0).strong());
            ui.add_space(6.0);
            ui.add_sized(
                [80.0, 26.0], // Smaller height
                egui::TextEdit::singleline(auto_sync_interval_str)
                    .font(egui::FontId::new(14.0, egui::FontFamily::Monospace))
                    .margin(egui::Margin::symmetric(6.0, 5.0)),
            );
            ui.label("min");

            // Update the actual setting
            if let Ok(value) = auto_sync_interval_str.parse::<u32>() {
                if value > 0 {
                    settings.auto_sync_interval_minutes = value;
                }
            }
        });

        // Requirements - always visible
        ui.add_space(8.0);
        ui.separator();
        ui.add_space(8.0);

        ui.vertical(|ui| {
            ui.label(egui::RichText::new("Requirements:").size(12.0).strong());
            ui.add_space(3.0);

            let host_ok = !settings.host.is_empty();
            let flow_ok = !settings.flow.is_empty();
            let in_place_ok = settings.in_place;

            ui.horizontal(|ui| {
                let (icon, color) = if host_ok {
                    ("✅", egui::Color32::GREEN)
                } else {
                    ("❌", egui::Color32::RED)
                };
                ui.label(egui::RichText::new(icon).color(color));
                ui.label(egui::RichText::new("Host URL").size(11.0));
            });

            ui.horizontal(|ui| {
                let (icon, color) = if flow_ok {
                    ("✅", egui::Color32::GREEN)
                } else {
                    ("❌", egui::Color32::RED)
                };
                ui.label(egui::RichText::new(icon).color(color));
                ui.label(egui::RichText::new("Flow name").size(11.0));
            });

            ui.horizontal(|ui| {
                let (icon, color) = if in_place_ok {
                    ("✅", egui::Color32::GREEN)
                } else {
                    ("❌", egui::Color32::RED)
                };
                ui.label(egui::RichText::new(icon).color(color));
                ui.label(egui::RichText::new("In-place update").size(11.0));
            });
        });
    });

    ui.add_space(8.0);
}

/// System information card
fn render_system_info_card(ui: &mut egui::Ui) {
    let frame = egui::Frame::group(ui.style())
        .fill(ui.visuals().extreme_bg_color)
        .stroke(egui::Stroke::new(
            1.0,
            ui.visuals().widgets.noninteractive.bg_stroke.color,
        ))
        .rounding(6.0)
        .inner_margin(egui::Margin::same(12.0));

    frame.show(ui, |ui| {
        // Header
        ui.horizontal(|ui| {
            ui.label("🔧");
            ui.label(egui::RichText::new("System Info").size(14.0).strong());
        });

        ui.add_space(8.0);

        // Config file location
        ui.vertical(|ui| {
            ui.label(egui::RichText::new("Config File").size(13.0).strong());
            ui.add_space(3.0);

            let config_path = get_config_path();
            let path_str = config_path.display().to_string();

            ui.vertical(|ui| {
                ui.add_sized(
                    [ui.available_width(), 26.0], // Smaller height
                    egui::TextEdit::singleline(&mut path_str.clone())
                        .interactive(false)
                        .font(egui::FontId::new(12.0, egui::FontFamily::Monospace))
                        .margin(egui::Margin::symmetric(8.0, 5.0)),
                );

                ui.add_space(4.0);

                if ui.small_button("📋 Copy Path").clicked() {
                    ui.output_mut(|o| o.copied_text = path_str);
                }
            });
        });
    });

    ui.add_space(8.0);
}

/// Action section with buttons only (Activity Log moved to bottom panel)
fn render_action_section(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    settings: &KhmSettings,
    connection_tab: &mut ConnectionTab,
    operation_log: &mut Vec<String>,
) {
    ui.add_space(2.0);

    // Validation for save button
    let save_enabled = !settings.host.is_empty() && !settings.flow.is_empty();

    // Action buttons with modern styling
    render_modern_action_buttons(
        ui,
        ctx,
        settings,
        connection_tab,
        save_enabled,
        operation_log,
    );
}

/// Modern action buttons with improved styling and layout
fn render_modern_action_buttons(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    settings: &KhmSettings,
    connection_tab: &mut ConnectionTab,
    save_enabled: bool,
    operation_log: &mut Vec<String>,
) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;

        // Primary actions (left side)
        let mut save_button = ui.add_enabled(
            save_enabled,
            egui::Button::new(
                egui::RichText::new("💾 Save & Close")
                    .size(13.0)
                    .color(egui::Color32::WHITE)
            )
            .fill(if save_enabled {
                egui::Color32::from_rgb(0, 120, 212)
            } else {
                ui.visuals().widgets.inactive.bg_fill
            })
            .min_size(egui::vec2(120.0, 32.0))
            .rounding(6.0)
        );

        // Add tooltip when button is disabled
        if !save_enabled {
            save_button = save_button.on_hover_text("Complete server configuration to enable saving:\n• Host URL is required\n• Flow name is required");
        }

        if save_button.clicked() {
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
            egui::Button::new(
                egui::RichText::new("✖ Cancel")
                    .size(13.0)
                    .color(ui.visuals().text_color())
            )
            .stroke(egui::Stroke::new(1.0, ui.visuals().text_color()))
            .fill(egui::Color32::TRANSPARENT)
            .min_size(egui::vec2(80.0, 32.0))
            .rounding(6.0)
        ).clicked() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }

        // Spacer
        ui.add_space(ui.available_width() - 220.0);

        // Secondary actions (right side)
        let can_test = !settings.host.is_empty() && !settings.flow.is_empty() && !connection_tab.is_testing_connection;
        let can_sync = !settings.host.is_empty() && !settings.flow.is_empty() && !connection_tab.is_syncing;

        if ui.add_enabled(
            can_test,
            egui::Button::new(
                egui::RichText::new(
                    if connection_tab.is_testing_connection {
                        "🔄 Testing..."
                    } else {
                        "🔍 Test"
                    }
                )
                .size(13.0)
                .color(egui::Color32::WHITE)
            )
            .fill(if can_test {
                egui::Color32::from_rgb(16, 124, 16)
            } else {
                ui.visuals().widgets.inactive.bg_fill
            })
            .min_size(egui::vec2(80.0, 32.0))
            .rounding(6.0)
        ).on_hover_text("Test server connection").clicked() {
            add_log_entry(operation_log, "🔍 Testing connection...".to_string());
            connection_tab.start_test(settings, ctx);
        }

        if ui.add_enabled(
            can_sync,
            egui::Button::new(
                egui::RichText::new(
                    if connection_tab.is_syncing {
                        "🔄 Syncing..."
                    } else {
                        "🔄 Sync"
                    }
                )
                .size(13.0)
                .color(egui::Color32::WHITE)
            )
            .fill(if can_sync {
                egui::Color32::from_rgb(255, 140, 0)
            } else {
                ui.visuals().widgets.inactive.bg_fill
            })
            .min_size(egui::vec2(80.0, 32.0))
            .rounding(6.0)
        ).on_hover_text("Synchronize SSH keys now").clicked() {
            add_log_entry(operation_log, "🔄 Starting sync...".to_string());
            connection_tab.start_sync(settings, ctx);
        }
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
