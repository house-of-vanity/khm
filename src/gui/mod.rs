use log::info;

// Modules
mod api;
mod admin;
mod common;

#[cfg(feature = "gui")]
mod settings;
#[cfg(feature = "gui")]
mod tray;

// Re-exports for backward compatibility and external usage
#[cfg(feature = "gui")]
pub use settings::run_settings_window;
#[cfg(feature = "gui")]
pub use tray::run_tray_app;

// User events for GUI communication
#[cfg(feature = "gui")]
#[derive(Debug)]
pub enum UserEvent {
    TrayIconEvent,
    MenuEvent(tray_icon::menu::MenuEvent),
    ConfigFileChanged,
    UpdateMenu,
}

/// Run GUI application in tray mode
#[cfg(feature = "gui")]
pub async fn run_gui() -> std::io::Result<()> {
    info!("Starting KHM tray application");
    run_tray_app().await
}

/// Stub function when GUI is disabled
#[cfg(not(feature = "gui"))]
pub async fn run_gui() -> std::io::Result<()> {
    return Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "GUI features not compiled. Install system dependencies and rebuild with --features gui"
    ));
}
