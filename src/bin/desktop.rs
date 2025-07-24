use khm::{gui, Args};

use clap::Parser;
use env_logger;
use log::{error, info};

/// Desktop version of KHM - Known Hosts Manager with GUI interface
/// Primarily runs in GUI mode with tray application and settings windows
#[derive(Parser, Debug, Clone)]
#[command(
    author = env!("CARGO_PKG_AUTHORS"),
    version = env!("CARGO_PKG_VERSION"),
    about = "SSH Host Key Manager (Desktop)",
    long_about = None,
    after_help = "Examples:\n\
    \n\
    Running in GUI tray mode (default):\n\
    khm-desktop\n\
    \n\
    Running in GUI tray mode with background daemon:\n\
    khm-desktop --daemon\n\
    \n\
    Running settings window:\n\
    khm-desktop --settings-ui\n\
    \n\
    "
)]
pub struct DesktopArgs {
    /// Hide console window and run in background (default: auto when no arguments)
    #[arg(long, help = "Hide console window and run in background")]
    pub daemon: bool,

    /// Run settings UI window
    #[arg(long, help = "Run settings UI window")]
    pub settings_ui: bool,
}

impl From<DesktopArgs> for Args {
    fn from(desktop_args: DesktopArgs) -> Self {
        Args {
            server: false,
            daemon: desktop_args.daemon,
            settings_ui: desktop_args.settings_ui,
            in_place: false,
            flows: vec!["default".to_string()],
            ip: "127.0.0.1".to_string(),
            port: 8080,
            db_host: "127.0.0.1".to_string(),
            db_name: "khm".to_string(),
            db_user: None,
            db_password: None,
            host: None,
            flow: None,
            known_hosts: "~/.ssh/known_hosts".to_string(),
            basic_auth: String::new(),
        }
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Configure logging to show only khm logs, filtering out noisy library logs
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Warn) // Default level for all modules
        .filter_module("khm", log::LevelFilter::Debug) // Our app logs
        .filter_module("winit", log::LevelFilter::Error) // Window management
        .filter_module("egui", log::LevelFilter::Error) // GUI framework
        .filter_module("eframe", log::LevelFilter::Error) // GUI framework
        .filter_module("tray_icon", log::LevelFilter::Error) // Tray icon
        .filter_module("wgpu", log::LevelFilter::Error) // Graphics
        .filter_module("naga", log::LevelFilter::Error) // Graphics
        .filter_module("glow", log::LevelFilter::Error) // Graphics
        .filter_module("tracing", log::LevelFilter::Error) // Tracing spans
        .init();

    info!("Starting SSH Key Manager (Desktop)");

    let desktop_args = DesktopArgs::parse();
    let args: Args = desktop_args.into();

    // Hide console on Windows if daemon flag is set
    if args.daemon {
        #[cfg(target_os = "windows")]
        {
            extern "system" {
                fn FreeConsole() -> i32;
            }
            unsafe {
                FreeConsole();
            }
        }
    }

    // Settings UI mode - just show settings window and exit
    if args.settings_ui {
        // Always hide console for settings window
        #[cfg(target_os = "windows")]
        {
            extern "system" {
                fn FreeConsole() -> i32;
            }
            unsafe {
                FreeConsole();
            }
        }
        
        #[cfg(feature = "gui")]
        {
            info!("Running settings UI window");
            gui::run_settings_window();
            return Ok(());
        }
        #[cfg(not(feature = "gui"))]
        {
            error!("GUI features not compiled. Install system dependencies and rebuild with --features gui");
            return Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "GUI features not compiled",
            ));
        }
    }

    // Default to GUI mode for desktop version
    info!("Running in GUI mode");
    #[cfg(feature = "gui")]
    {
        if let Err(e) = gui::run_gui().await {
            error!("Failed to run GUI: {}", e);
        }
    }
    #[cfg(not(feature = "gui"))]
    {
        error!("GUI features not compiled. Install system dependencies and rebuild with --features gui");
        return Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "GUI features not compiled",
        ));
    }

    info!("Application has exited");
    Ok(())
}