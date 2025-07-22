use eframe::egui;
use std::collections::BTreeMap;
use super::state::{AdminState, get_key_type, get_key_preview};
use crate::gui::api::SshKey;

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
                        ui.label(egui::RichText::new(stats.total_keys.to_string()).size(24.0).strong());
                        ui.label(egui::RichText::new("Total Keys").size(11.0).color(egui::Color32::GRAY));
                    });
                    
                    // Active keys
                    cols[1].vertical_centered_justified(|ui| {
                        ui.label(egui::RichText::new("✅").size(20.0));
                        ui.label(egui::RichText::new(stats.active_keys.to_string()).size(24.0).strong().color(egui::Color32::LIGHT_GREEN));
                        ui.label(egui::RichText::new("Active").size(11.0).color(egui::Color32::GRAY));
                    });
                    
                    // Deprecated keys
                    cols[2].vertical_centered_justified(|ui| {
                        ui.label(egui::RichText::new("❌").size(20.0));
                        ui.label(egui::RichText::new(stats.deprecated_keys.to_string()).size(24.0).strong().color(egui::Color32::LIGHT_RED));
                        ui.label(egui::RichText::new("Deprecated").size(11.0).color(egui::Color32::GRAY));
                    });
                    
                    // Servers
                    cols[3].vertical_centered_justified(|ui| {
                        ui.label(egui::RichText::new("💻").size(20.0));
                        ui.label(egui::RichText::new(stats.unique_servers.to_string()).size(24.0).strong().color(egui::Color32::LIGHT_BLUE));
                        ui.label(egui::RichText::new("Servers").size(11.0).color(egui::Color32::GRAY));
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
            ui.label(egui::RichText::new("🔍 Search").size(16.0).strong());
            ui.add_space(8.0);
            
            // Search field with full width
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("🔍").size(14.0));
                let search_response = ui.add_sized(
                    [ui.available_width() * 0.6, 20.0],
                    egui::TextEdit::singleline(&mut admin_state.search_term)
                        .hint_text("Search servers or keys...")
                );
                
                if admin_state.search_term.is_empty() {
                    ui.label(egui::RichText::new("Type to search").size(11.0).color(egui::Color32::GRAY));
                } else {
                    ui.label(egui::RichText::new(format!("{} results", admin_state.filtered_keys.len())).size(11.0));
                    if ui.add(egui::Button::new(egui::RichText::new("❌").color(egui::Color32::WHITE))
                        .fill(egui::Color32::from_rgb(170, 170, 170))
                        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(89, 89, 89)))
                        .rounding(egui::Rounding::same(3.0))
                        .min_size(egui::vec2(18.0, 18.0))
                    ).on_hover_text("Clear search").clicked() {
                        admin_state.search_term.clear();
                        changed = true;
                    }
                }
                
                // Handle search text changes
                if search_response.changed() {
                    changed = true;
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
    let selected_count = admin_state.selected_servers.values().filter(|&&v| v).count();
    
    if selected_count == 0 {
        return BulkAction::None;
    }
    
    let mut action = BulkAction::None;
    
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
                    action = BulkAction::DeprecateSelected;
                }
                
                if ui.add(egui::Button::new(egui::RichText::new("✅ Restore Selected").color(egui::Color32::WHITE))
                    .fill(egui::Color32::from_rgb(101, 199, 40))
                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(94, 105, 25)))
                    .rounding(egui::Rounding::same(6.0))
                    .min_size(egui::vec2(120.0, 28.0))
                ).clicked() {
                    action = BulkAction::RestoreSelected;
                }
                
                if ui.add(egui::Button::new(egui::RichText::new("X Clear Selection").color(egui::Color32::WHITE))
                    .fill(egui::Color32::from_rgb(170, 170, 170))
                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(89, 89, 89)))
                    .rounding(egui::Rounding::same(6.0))
                    .min_size(egui::vec2(110.0, 28.0))
                ).clicked() {
                    admin_state.clear_selection();
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
    let mut servers: BTreeMap<String, Vec<SshKey>> = BTreeMap::new();
    for key in &admin_state.filtered_keys {
        servers.entry(key.server.clone()).or_insert_with(Vec::new).push(key.clone());
    }
    
    // Render each server group
    for (server_name, server_keys) in servers {
        let is_expanded = admin_state.expanded_servers.get(&server_name).copied().unwrap_or(false);
        let active_count = server_keys.iter().filter(|k| !k.deprecated).count();
        let deprecated_count = server_keys.len() - active_count;
        
        // Server header
        ui.group(|ui| {
            ui.horizontal(|ui| {
                // Server selection checkbox
                let mut selected = admin_state.selected_servers.get(&server_name).copied().unwrap_or(false);
                if ui.add(egui::Checkbox::new(&mut selected, "")
                    .indeterminate(false)
                ).changed() {
                    admin_state.selected_servers.insert(server_name.clone(), selected);
                }
                
                // Expand/collapse button
                let expand_icon = if is_expanded { "▼" } else { "▶" };
                if ui.add(egui::Button::new(expand_icon)
                    .fill(egui::Color32::TRANSPARENT)
                    .stroke(egui::Stroke::NONE)
                    .min_size(egui::vec2(20.0, 20.0))
                ).clicked() {
                    admin_state.expanded_servers.insert(server_name.clone(), !is_expanded);
                }
                
                // Server icon and name
                ui.label(egui::RichText::new("💻").size(16.0));
                ui.label(egui::RichText::new(&server_name)
                    .size(15.0)
                    .strong()
                    .color(egui::Color32::WHITE));
                
                // Keys count badge
                render_badge(ui, &format!("{} keys", server_keys.len()), egui::Color32::from_rgb(52, 152, 219), egui::Color32::WHITE);
                
                ui.add_space(5.0);
                
                // Deprecated count badge
                if deprecated_count > 0 {
                    render_badge(ui, &format!("{} depr", deprecated_count), egui::Color32::from_rgb(231, 76, 60), egui::Color32::WHITE);
                }
                
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Server action buttons
                    if deprecated_count > 0 {
                        if ui.add(egui::Button::new(egui::RichText::new("✅ Restore").color(egui::Color32::WHITE))
                            .fill(egui::Color32::from_rgb(101, 199, 40))
                            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(94, 105, 25)))
                            .rounding(egui::Rounding::same(4.0))
                            .min_size(egui::vec2(70.0, 24.0))
                        ).clicked() {
                            action = KeyAction::RestoreServer(server_name.clone());
                        }
                    }
                    
                    if active_count > 0 {
                        if ui.add(egui::Button::new(egui::RichText::new("❗ Deprecate").color(egui::Color32::BLACK))
                            .fill(egui::Color32::from_rgb(255, 200, 0))
                            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(102, 94, 72)))
                            .rounding(egui::Rounding::same(4.0))
                            .min_size(egui::vec2(85.0, 24.0))
                        ).clicked() {
                            action = KeyAction::DeprecateServer(server_name.clone());
                        }
                    }
                });
            });
        });
        
        // Expanded key details
        if is_expanded {
            ui.indent("server_keys", |ui| {
                for key in &server_keys {
                    if let Some(key_action) = render_key_item(ui, key, &server_name) {
                        action = key_action;
                    }
                }
            });
        }
        
        ui.add_space(5.0);
    }
    
    action
}

/// Render empty state when no keys are available
fn render_empty_state(ui: &mut egui::Ui, admin_state: &AdminState) {
    ui.vertical_centered(|ui| {
        ui.add_space(60.0);
        if admin_state.keys.is_empty() {
            ui.label(egui::RichText::new("🔑").size(48.0).color(egui::Color32::GRAY));
            ui.label(egui::RichText::new("No SSH keys available")
                .size(18.0)
                .color(egui::Color32::GRAY));
            ui.label(egui::RichText::new("Keys will appear here once loaded from the server")
                .size(14.0)
                .color(egui::Color32::DARK_GRAY));
        } else if !admin_state.search_term.is_empty() {
            ui.label(egui::RichText::new("🔍").size(48.0).color(egui::Color32::GRAY));
            ui.label(egui::RichText::new("No results found")
                .size(18.0)
                .color(egui::Color32::GRAY));
            ui.label(egui::RichText::new(format!("Try adjusting your search: '{}'", admin_state.search_term))
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
}

/// Render individual key item
fn render_key_item(ui: &mut egui::Ui, key: &SshKey, server_name: &str) -> Option<KeyAction> {
    let mut action = None;
    
    ui.group(|ui| {
        ui.horizontal(|ui| {
            // Key type badge
            let key_type = get_key_type(&key.public_key);
            let (badge_color, text_color) = match key_type.as_str() {
                "RSA" => (egui::Color32::from_rgb(52, 144, 220), egui::Color32::WHITE),
                "ED25519" => (egui::Color32::from_rgb(46, 204, 113), egui::Color32::WHITE),
                "ECDSA" => (egui::Color32::from_rgb(241, 196, 15), egui::Color32::BLACK),
                "DSA" => (egui::Color32::from_rgb(230, 126, 34), egui::Color32::WHITE),
                _ => (egui::Color32::GRAY, egui::Color32::WHITE),
            };
            
            render_small_badge(ui, &key_type, badge_color, text_color);
            ui.add_space(5.0);
            
            // Status badge
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
            
            // Key preview
            ui.label(egui::RichText::new(get_key_preview(&key.public_key))
                .font(egui::FontId::monospace(10.0))
                .color(egui::Color32::LIGHT_GRAY));
            
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Key action buttons
                if key.deprecated {
                    if ui.add(egui::Button::new(egui::RichText::new("[R]").color(egui::Color32::WHITE))
                        .fill(egui::Color32::from_rgb(101, 199, 40))
                        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(94, 105, 25)))
                        .rounding(egui::Rounding::same(3.0))
                        .min_size(egui::vec2(22.0, 18.0))
                    ).on_hover_text("Restore key").clicked() {
                        action = Some(KeyAction::RestoreKey(server_name.to_string()));
                    }
                    if ui.add(egui::Button::new(egui::RichText::new("Del").color(egui::Color32::WHITE))
                        .fill(egui::Color32::from_rgb(246, 36, 71))
                        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(129, 18, 17)))
                        .rounding(egui::Rounding::same(3.0))
                        .min_size(egui::vec2(26.0, 18.0))
                    ).on_hover_text("Delete key").clicked() {
                        action = Some(KeyAction::DeleteKey(server_name.to_string()));
                    }
                } else {
                    if ui.add(egui::Button::new(egui::RichText::new("❗").color(egui::Color32::BLACK))
                        .fill(egui::Color32::from_rgb(255, 200, 0))
                        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(102, 94, 72)))
                        .rounding(egui::Rounding::same(3.0))
                        .min_size(egui::vec2(22.0, 18.0))
                    ).on_hover_text("Deprecate key").clicked() {
                        action = Some(KeyAction::DeprecateKey(server_name.to_string()));
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
    
    action
}

/// Render a badge with text
fn render_badge(ui: &mut egui::Ui, text: &str, bg_color: egui::Color32, text_color: egui::Color32) {
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(50.0, 18.0),
        egui::Sense::hover()
    );
    ui.painter().rect_filled(
        rect,
        egui::Rounding::same(8.0),
        bg_color
    );
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        text,
        egui::FontId::proportional(10.0),
        text_color,
    );
}

/// Render a small badge with text
fn render_small_badge(ui: &mut egui::Ui, text: &str, bg_color: egui::Color32, text_color: egui::Color32) {
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(40.0, 16.0),
        egui::Sense::hover()
    );
    ui.painter().rect_filled(
        rect,
        egui::Rounding::same(3.0),
        bg_color
    );
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        text,
        egui::FontId::proportional(9.0),
        text_color,
    );
}

/// Actions that can be performed on keys
#[derive(Debug, Clone)]
pub enum KeyAction {
    None,
    DeprecateKey(String),
    RestoreKey(String),
    DeleteKey(String),
    DeprecateServer(String),
    RestoreServer(String),
}

/// Bulk actions that can be performed
#[derive(Debug, Clone)]
pub enum BulkAction {
    None,
    DeprecateSelected,
    RestoreSelected,
    ClearSelection,
}
