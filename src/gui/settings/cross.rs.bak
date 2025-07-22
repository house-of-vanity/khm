use super::{load_settings, save_settings, KhmSettings};
use eframe::egui;
use log::error;

struct KhmSettingsWindow {
    settings: KhmSettings,
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
                ui.label("Auto sync interval (minutes):");
                ui.add(egui::DragValue::new(&mut self.settings.auto_sync_interval_minutes).range(5..=1440));
            });
            
            ui.checkbox(&mut self.settings.in_place, "Update known_hosts file in-place after sync");
            
            ui.separator();
            
            ui.horizontal(|ui| {
                if ui.button("Save").clicked() {
                    if let Err(e) = save_settings(&self.settings) {
                        error!("Failed to save KHM settings: {}", e);
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
    
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("KHM Settings")
            .with_inner_size([450.0, 385.0]),
        ..Default::default()
    };
    
    let _ = eframe::run_native(
        "KHM Settings",
        options,
        Box::new(|_cc| Ok(Box::new(KhmSettingsWindow { settings }))),
    );
}
