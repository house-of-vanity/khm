use super::state::{AdminState, AdminSettings, ConnectionStatus, get_key_type, get_key_preview};
use eframe::egui;
use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub enum KeyAction {
    None,
    DeprecateKey(String),
    RestoreKey(String),
    DeleteKey(String),
    DeprecateServer(String),
    RestoreServer(String),
}

#[derive(Debug, Clone)]
pub enum BulkAction {
    None,
    DeprecateSelected,
    RestoreSelected,
    ClearSelection,
}

/// Render connection settings panel
pub fn render_connection_settings(
    ui: &mut egui::Ui,
    settings: &mut AdminSettings,
    connection_status: &ConnectionStatus,
    flows: &[String],
    server_version: &Option<String>,
) -> ConnectionAction {
    let mut action = ConnectionAction::None;
    
    ui.group(|ui| {
        ui.set_min_width(ui.available_width());
        ui.vertical(|ui| {
            ui.label(egui::RichText::new("⚙️ Connection Settings").size(16.0).strong());
            ui.add_space(8.0);
            
            // Server URL
            ui.horizontal(|ui| {
                ui.label("Server URL:");
                ui.text_edit_singleline(&mut settings.server_url);
            });
            
            // Basic Auth
            ui.horizontal(|ui| {
                ui.label("Basic Auth:");
                ui.add(egui::TextEdit::singleline(&mut settings.basic_auth).password(true));
            });
            
            // Flow selection
            ui.horizontal(|ui| {
                ui.label("Flow:");
                egui::ComboBox::from_id_salt("flow_select")
                    .selected_text(&settings.selected_flow)
                    .show_ui(ui, |ui| {
                        for flow in flows {
                            ui.selectable_value(&mut settings.selected_flow, flow.clone(), flow);
                        }
                    });
            });
            
            // Connection status
            ui.horizontal(|ui| {
                ui.label("Status:");
                match connection_status {
                    ConnectionStatus::Connected => {
                        ui.colored_label(egui::Color32::GREEN, "● Connected");
                    }
                    ConnectionStatus::Connecting => {
                        ui.colored_label(egui::Color32::YELLOW, "● Connecting...");
                    }
                    ConnectionStatus::Disconnected => {
                        ui.colored_label(egui::Color32::GRAY, "● Disconnected");
                    }
                    ConnectionStatus::Error(msg) => {
                        ui.colored_label(egui::Color32::RED, format!("● Error: {}", msg));
                    }
                }
            });
            
            // Server version display
            if let Some(version) = server_version {
                ui.horizontal(|ui| {
                    ui.label("Server Version:");
                    ui.colored_label(egui::Color32::LIGHT_BLUE, version);
                });
            }
            
            ui.add_space(8.0);
            
            // Action buttons
            ui.horizontal(|ui| {
                if ui.button("Load Flows").clicked() {
                    action = ConnectionAction::LoadFlows;
                }
                
                if ui.button("Test Connection").clicked() {
                    action = ConnectionAction::TestConnection;
                }
                
                if ui.button("Get Version").clicked() {
                    action = ConnectionAction::LoadVersion;
                }
                
                if !settings.selected_flow.is_empty() && ui.button("Load Keys").clicked() {
                    action = ConnectionAction::LoadKeys;
                }
            });
        });
    });
    
    action
}

/// Render statistics cards
pub fn render_statistics(ui: &mut egui::Ui, admin_state: &AdminState) {
    let stats = admin_state.get_statistics();
    
    ui.group(|ui| {
        ui.set_min_width(ui.available_width());
        ui.vertical(|ui| {
            ui.label(egui::RichText::new("📊 Statistics").size(16.0).strong());
            ui.add_space(8.0);
            
            ui.horizontal(|ui| {
                ui.columns(4, |cols| {
                    // Total keys
                    cols[0].vertical_centered_justified(|ui| {
                        ui.label(egui::RichText::new("📊").size(20.0));
                        ui.label(
                            egui::RichText::new(stats.total_keys.to_string())
                                .size(24.0)
                                .strong(),
                        );
                        ui.label(
                            egui::RichText::new("Total Keys")
                                .size(11.0)
                                .color(egui::Color32::GRAY),
                        );
                    });
                    
                    // Active keys
                    cols[1].vertical_centered_justified(|ui| {
                        ui.label(egui::RichText::new("✅").size(20.0));
                        ui.label(
                            egui::RichText::new(stats.active_keys.to_string())
                                .size(24.0)
                                .strong()
                                .color(egui::Color32::LIGHT_GREEN),
                        );
                        ui.label(
                            egui::RichText::new("Active")
                                .size(11.0)
                                .color(egui::Color32::GRAY),
                        );
                    });
                    
                    // Deprecated keys
                    cols[2].vertical_centered_justified(|ui| {
                        ui.label(egui::RichText::new("❌").size(20.0));
                        ui.label(
                            egui::RichText::new(stats.deprecated_keys.to_string())
                                .size(24.0)
                                .strong()
                                .color(egui::Color32::LIGHT_RED),
                        );
                        ui.label(
                            egui::RichText::new("Deprecated")
                                .size(11.0)
                                .color(egui::Color32::GRAY),
                        );
                    });
                    
                    // Servers
                    cols[3].vertical_centered_justified(|ui| {
                        ui.label(egui::RichText::new("💻").size(20.0));
                        ui.label(
                            egui::RichText::new(stats.unique_servers.to_string())
                                .size(24.0)
                                .strong()
                                .color(egui::Color32::LIGHT_BLUE),
                        );
                        ui.label(
                            egui::RichText::new("Servers")
                                .size(11.0)
                                .color(egui::Color32::GRAY),
                        );
                    });
                });
            });
        });
    });
}

/// Render search and filter controls
pub fn render_search_controls(ui: &mut egui::Ui, admin_state: &mut AdminState) -> bool {
    let mut changed = false;
    
    ui.group(|ui| {
        ui.set_min_width(ui.available_width());
        ui.vertical(|ui| {
            ui.label(egui::RichText::new("🔍 Search & Filter").size(16.0).strong());
            ui.add_space(8.0);
            
            // Search field
            ui.horizontal(|ui| {
                ui.label("Search:");
                let search_response = ui.add_sized(
                    [ui.available_width() * 0.6, 20.0],
                    egui::TextEdit::singleline(&mut admin_state.search_term)
                        .hint_text("Search servers or keys..."),
                );
                
                if search_response.changed() {
                    changed = true;
                }
                
                if !admin_state.search_term.is_empty() {
                    if ui.small_button("Clear").clicked() {
                        admin_state.search_term.clear();
                        changed = true;
                    }
                }
            });
            
            ui.add_space(5.0);
            
            // Filter controls
            ui.horizontal(|ui| {
                ui.label("Filter:");
                let show_deprecated = admin_state.show_deprecated_only;
                if ui.selectable_label(!show_deprecated, "✅ Active").clicked() {
                    admin_state.show_deprecated_only = false;
                    changed = true;
                }
                if ui.selectable_label(show_deprecated, "❗ Deprecated").clicked() {
                    admin_state.show_deprecated_only = true;
                    changed = true;
                }
            });
        });
    });
    
    if changed {
        admin_state.filter_keys();
    }
    
    changed
}

/// Render bulk actions controls
pub fn render_bulk_actions(ui: &mut egui::Ui, admin_state: &mut AdminState) -> BulkAction {
    let selected_count = admin_state
        .selected_servers
        .values()
        .filter(|&&v| v)
        .count();
        
    if selected_count == 0 {
        return BulkAction::None;
    }
    
    let mut action = BulkAction::None;
    
    ui.group(|ui| {
        ui.set_min_width(ui.available_width());
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("📋").size(14.0));
                ui.label(
                    egui::RichText::new(format!("Selected {} servers", selected_count))
                        .size(14.0)
                        .strong()
                        .color(egui::Color32::LIGHT_BLUE),
                );
            });
            
            ui.add_space(5.0);
            
            ui.horizontal(|ui| {
                if ui.button("❗ Deprecate Selected").clicked() {
                    action = BulkAction::DeprecateSelected;
                }
                
                if ui.button("✅ Restore Selected").clicked() {
                    action = BulkAction::RestoreSelected;
                }
                
                if ui.button("Clear Selection").clicked() {
                    action = BulkAction::ClearSelection;
                }
            });
        });
    });
    
    action
}

/// Render keys table grouped by servers
pub fn render_keys_table(ui: &mut egui::Ui, admin_state: &mut AdminState) -> KeyAction {
    if admin_state.filtered_keys.is_empty() {
        render_empty_state(ui, admin_state);
        return KeyAction::None;
    }
    
    let mut action = KeyAction::None;
    
    // Group keys by server
    let mut servers: BTreeMap<String, Vec<&crate::web_gui::state::SshKey>> = BTreeMap::new();
    for key in &admin_state.filtered_keys {
        servers
            .entry(key.server.clone())
            .or_insert_with(Vec::new)
            .push(key);
    }
    
    // Render each server group
    egui::ScrollArea::vertical().show(ui, |ui| {
        for (server_name, server_keys) in servers {
            let is_expanded = admin_state
                .expanded_servers
                .get(&server_name)
                .copied()
                .unwrap_or(false);
            let active_count = server_keys.iter().filter(|k| !k.deprecated).count();
            let deprecated_count = server_keys.len() - active_count;
            
            // Server header
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    // Server selection checkbox
                    let mut selected = admin_state
                        .selected_servers
                        .get(&server_name)
                        .copied()
                        .unwrap_or(false);
                    if ui.checkbox(&mut selected, "").changed() {
                        admin_state
                            .selected_servers
                            .insert(server_name.clone(), selected);
                    }
                    
                    // Expand/collapse button
                    let expand_icon = if is_expanded { "▼" } else { "▶" };
                    if ui.small_button(expand_icon).clicked() {
                        admin_state
                            .expanded_servers
                            .insert(server_name.clone(), !is_expanded);
                    }
                    
                    // Server icon and name
                    ui.label(egui::RichText::new("💻").size(16.0));
                    ui.label(
                        egui::RichText::new(&server_name)
                            .size(15.0)
                            .strong(),
                    );
                    
                    // Keys count badge
                    ui.label(format!("({} keys)", server_keys.len()));
                    
                    // Deprecated count badge
                    if deprecated_count > 0 {
                        ui.colored_label(
                            egui::Color32::RED,
                            format!("{} deprecated", deprecated_count)
                        );
                    }
                    
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // Server action buttons
                        if deprecated_count > 0 {
                            if ui.small_button("✅ Restore").clicked() {
                                action = KeyAction::RestoreServer(server_name.clone());
                            }
                        }
                        
                        if active_count > 0 {
                            if ui.small_button("❗ Deprecate").clicked() {
                                action = KeyAction::DeprecateServer(server_name.clone());
                            }
                        }
                    });
                });
            });
            
            // Expanded key details
            if is_expanded {
                ui.indent(&server_name, |ui| {
                    for key in &server_keys {
                        if let Some(key_action) = render_key_item(ui, key, &server_name) {
                            action = key_action;
                        }
                    }
                });
            }
            
            ui.add_space(5.0);
        }
    });
    
    action
}

/// Render empty state when no keys are available
fn render_empty_state(ui: &mut egui::Ui, admin_state: &AdminState) {
    ui.vertical_centered(|ui| {
        ui.add_space(60.0);
        if admin_state.keys.is_empty() {
            ui.label(
                egui::RichText::new("🔑")
                    .size(48.0)
                    .color(egui::Color32::GRAY),
            );
            ui.label(
                egui::RichText::new("No SSH keys available")
                    .size(18.0)
                    .color(egui::Color32::GRAY),
            );
            ui.label(
                egui::RichText::new("Keys will appear here once loaded from the server")
                    .size(14.0)
                    .color(egui::Color32::DARK_GRAY),
            );
        } else if !admin_state.search_term.is_empty() {
            ui.label(
                egui::RichText::new("🔍")
                    .size(48.0)
                    .color(egui::Color32::GRAY),
            );
            ui.label(
                egui::RichText::new("No results found")
                    .size(18.0)
                    .color(egui::Color32::GRAY),
            );
            ui.label(
                egui::RichText::new(format!(
                    "Try adjusting your search: '{}'",
                    admin_state.search_term
                ))
                .size(14.0)
                .color(egui::Color32::DARK_GRAY),
            );
        } else {
            ui.label(
                egui::RichText::new("❌")
                    .size(48.0)
                    .color(egui::Color32::GRAY),
            );
            ui.label(
                egui::RichText::new("No keys match current filters")
                    .size(18.0)
                    .color(egui::Color32::GRAY),
            );
            ui.label(
                egui::RichText::new("Try adjusting your search or filter settings")
                    .size(14.0)
                    .color(egui::Color32::DARK_GRAY),
            );
        }
    });
}

/// Render individual key item
fn render_key_item(
    ui: &mut egui::Ui,
    key: &crate::web_gui::state::SshKey,
    server_name: &str,
) -> Option<KeyAction> {
    let mut action = None;
    
    ui.group(|ui| {
        ui.horizontal(|ui| {
            // Key type badge
            let key_type = get_key_type(&key.public_key);
            let badge_color = match key_type.as_str() {
                "RSA" => egui::Color32::from_rgb(52, 144, 220),
                "ED25519" => egui::Color32::from_rgb(46, 204, 113),
                "ECDSA" => egui::Color32::from_rgb(241, 196, 15),
                "DSA" => egui::Color32::from_rgb(230, 126, 34),
                _ => egui::Color32::GRAY,
            };
            
            ui.colored_label(badge_color, &key_type);
            ui.add_space(5.0);
            
            // Status badge
            if key.deprecated {
                ui.colored_label(egui::Color32::RED, "❗ DEPRECATED");
            } else {
                ui.colored_label(egui::Color32::GREEN, "✅ ACTIVE");
            }
            
            ui.add_space(5.0);
            
            // Key preview
            ui.monospace(get_key_preview(&key.public_key));
            
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Key action buttons
                if key.deprecated {
                    if ui.small_button("Restore").clicked() {
                        action = Some(KeyAction::RestoreKey(server_name.to_string()));
                    }
                    if ui.small_button("Delete").clicked() {
                        action = Some(KeyAction::DeleteKey(server_name.to_string()));
                    }
                } else {
                    if ui.small_button("Deprecate").clicked() {
                        action = Some(KeyAction::DeprecateKey(server_name.to_string()));
                    }
                }
                
                if ui.small_button("Copy").clicked() {
                    ui.output_mut(|o| o.copied_text = key.public_key.clone());
                }
            });
        });
    });
    
    action
}

#[derive(Debug, Clone)]
pub enum ConnectionAction {
    None,
    LoadFlows,
    TestConnection,
    LoadKeys,
    LoadVersion,
}