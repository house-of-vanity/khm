mod client;
mod db;
mod server;
mod web;
mod gui;

use clap::Parser;
use env_logger;
use log::{error, info};

/// This application manages SSH keys and flows, either as a server or client.
/// In server mode, it stores keys and flows in a PostgreSQL database.
/// In client mode, it sends keys to the server and can update the known_hosts file with keys from the server.
#[derive(Parser, Debug, Clone)]
#[command(
    author = env!("CARGO_PKG_AUTHORS"),
    version = env!("CARGO_PKG_VERSION"),
    about = "SSH Host Key Manager",
    long_about = None,
    after_help = "Examples:\n\
    \n\
    Running in server mode:\n\
    khm --server --ip 0.0.0.0 --port 1337 --db-host psql.psql.svc --db-name khm --db-user admin --db-password <SECRET> --flows work,home\n\
    \n\
    Running in client mode to send diff and sync ~/.ssh/known_hosts with remote flow `work` in place:\n\
    khm --host https://khm.example.com --flow work --known-hosts ~/.ssh/known_hosts --in-place\n\
    \n\
    "
)]
pub struct Args {
    /// Run in server mode (default: false)
    #[arg(long, help = "Run in server mode")]
    server: bool,

    /// Run with GUI tray interface (default: false)
    #[arg(long, help = "Run with GUI tray interface")]
    gui: bool,

    /// Run settings UI window (used with --gui)
    #[arg(long, help = "Run settings UI window (used with --gui)")]
    settings_ui: bool,

    /// Update the known_hosts file with keys from the server after sending keys (default: false)
    #[arg(
        long,
        help = "Server mode: Sync the known_hosts file with keys from the server"
    )]
    in_place: bool,

    /// Comma-separated list of flows to manage (default: default)
    #[arg(long, default_value = "default", value_parser, num_args = 1.., value_delimiter = ',', help = "Server mode: Comma-separated list of flows to manage")]
    flows: Vec<String>,

    /// IP address to bind the server or client to (default: 127.0.0.1)
    #[arg(
        short,
        long,
        default_value = "127.0.0.1",
        help = "Server mode: IP address to bind the server to"
    )]
    ip: String,

    /// Port to bind the server or client to (default: 8080)
    #[arg(
        short,
        long,
        default_value = "8080",
        help = "Server mode: Port to bind the server to"
    )]
    port: u16,

    /// Hostname or IP address of the PostgreSQL database (default: 127.0.0.1)
    #[arg(
        long,
        default_value = "127.0.0.1",
        help = "Server mode: Hostname or IP address of the PostgreSQL database"
    )]
    db_host: String,

    /// Name of the PostgreSQL database (default: khm)
    #[arg(
        long,
        default_value = "khm",
        help = "Server mode: Name of the PostgreSQL database"
    )]
    db_name: String,

    /// Username for the PostgreSQL database (required in server mode)
    #[arg(
        long,
        required_if_eq("server", "true"),
        help = "Server mode: Username for the PostgreSQL database"
    )]
    db_user: Option<String>,

    /// Password for the PostgreSQL database (required in server mode)
    #[arg(
        long,
        required_if_eq("server", "true"),
        help = "Server mode: Password for the PostgreSQL database"
    )]
    db_password: Option<String>,

    /// Host address of the server to connect to in client mode (required in client mode)
    #[arg(
        long,
        required_if_eq("server", "false"),
        help = "Client mode: Full host address of the server to connect to. Like https://khm.example.com"
    )]
    host: Option<String>,

    /// Flow name to use on the server
    #[arg(
        long,
        required_if_eq("server", "false"),
        help = "Client mode: Flow name to use on the server"
    )]
    flow: Option<String>,

    /// Path to the known_hosts file (default: ~/.ssh/known_hosts)
    #[arg(
        long,
        default_value = "~/.ssh/known_hosts",
        help = "Client mode: Path to the known_hosts file"
    )]
    known_hosts: String,

    /// Basic auth string for client mode. Format: user:pass
    #[arg(long, default_value = "", help = "Client mode: Basic Auth credentials")]
    basic_auth: String,
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Configure logging to show only khm logs, filtering out noisy library logs
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Warn) // Default level for all modules
        .filter_module("khm", log::LevelFilter::Debug) // Our app logs
        .filter_module("actix_web", log::LevelFilter::Info) // Server logs
        .filter_module("reqwest", log::LevelFilter::Warn) // HTTP client
        .filter_module("winit", log::LevelFilter::Error) // Window management
        .filter_module("egui", log::LevelFilter::Error) // GUI framework
        .filter_module("eframe", log::LevelFilter::Error) // GUI framework
        .filter_module("tray_icon", log::LevelFilter::Error) // Tray icon
        .filter_module("wgpu", log::LevelFilter::Error) // Graphics
        .filter_module("naga", log::LevelFilter::Error) // Graphics
        .filter_module("glow", log::LevelFilter::Error) // Graphics
        .filter_module("tracing", log::LevelFilter::Error) // Tracing spans
        .init();
    
    info!("Starting SSH Key Manager");

    let args = Args::parse();

    // Settings UI mode - just show settings window and exit
    if args.settings_ui {
        info!("Running settings UI window");
        gui::run_settings_window();
        return Ok(());
    }

    // GUI mode has priority
    if args.gui {
        info!("Running in GUI mode");
        if let Err(e) = gui::run_gui().await {
            error!("Failed to run GUI: {}", e);
        }
        return Ok(());
    }

    // Check if we have the minimum required arguments for server/client mode
    if !args.server && !args.gui && (args.host.is_none() || args.flow.is_none()) {
        // Neither server mode nor client mode nor GUI mode properly configured
        eprintln!("Error: You must specify either server mode (--server), client mode (--host and --flow), or GUI mode (--gui)");
        eprintln!();
        eprintln!("Examples:");
        eprintln!(
            "  Server mode: {} --server --db-user admin --db-password pass --flows work,home",
            env!("CARGO_PKG_NAME")
        );
        eprintln!(
            "  Client mode: {} --host https://khm.example.com --flow work",
            env!("CARGO_PKG_NAME")
        );
        eprintln!(
            "  GUI mode: {} --gui",
            env!("CARGO_PKG_NAME")
        );
        eprintln!(
            "  Settings window: {} --gui --settings-ui",
            env!("CARGO_PKG_NAME")
        );
        eprintln!();
        eprintln!("Use --help for more information.");
        std::process::exit(1);
    }

    if args.server {
        info!("Running in server mode");
        if let Err(e) = server::run_server(args).await {
            error!("Failed to run server: {}", e);
        }
    } else {
        info!("Running in client mode");
        if let Err(e) = client::run_client(args).await {
            error!("Failed to run client: {}", e);
        }
    }

    info!("Application has exited");
    Ok(())
}
