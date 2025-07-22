use log::info;
use tray_icon::menu::MenuEvent;

// Modules
mod api;
mod admin;
mod common;
mod settings;
mod tray;

// Re-exports for backward compatibility and external usage
pub use settings::run_settings_window;
pub use tray::run_tray_app;

// User events for GUI communication
#[derive(Debug)]
pub enum UserEvent {
    TrayIconEvent,
    MenuEvent(MenuEvent),
    ConfigFileChanged,
    UpdateMenu,
}

/// Run GUI application in tray mode
pub async fn run_gui() -> std::io::Result<()> {
    info!("Starting KHM tray application");
    run_tray_app().await
}
