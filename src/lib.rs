pub mod client;
pub mod db;
pub mod gui;
pub mod server;
#[cfg(feature = "web")]
pub mod web;
#[cfg(feature = "web-gui")]
pub mod web_gui;

use clap::Parser;

// Common Args structure used by all binaries
#[derive(Parser, Debug, Clone)]
pub struct Args {
    /// Run in server mode (default: false)
    #[arg(long, help = "Run in server mode")]
    pub server: bool,

    /// Hide console window and run in background (default: auto when no arguments)
    #[arg(long, help = "Hide console window and run in background")]
    pub daemon: bool,

    /// Run settings UI window
    #[arg(long, help = "Run settings UI window")]
    pub settings_ui: bool,

    /// Update the known_hosts file with keys from the server after sending keys (default: false)
    #[arg(
        long,
        help = "Server mode: Sync the known_hosts file with keys from the server"
    )]
    pub in_place: bool,

    /// Comma-separated list of flows to manage (default: default)
    #[arg(long, default_value = "default", value_parser, num_args = 1.., value_delimiter = ',', help = "Server mode: Comma-separated list of flows to manage")]
    pub flows: Vec<String>,

    /// IP address to bind the server or client to (default: 127.0.0.1)
    #[arg(
        short,
        long,
        default_value = "127.0.0.1",
        help = "Server mode: IP address to bind the server to"
    )]
    pub ip: String,

    /// Port to bind the server or client to (default: 8080)
    #[arg(
        short,
        long,
        default_value = "8080",
        help = "Server mode: Port to bind the server to"
    )]
    pub port: u16,

    /// Hostname or IP address of the PostgreSQL database (default: 127.0.0.1)
    #[arg(
        long,
        default_value = "127.0.0.1",
        help = "Server mode: Hostname or IP address of the PostgreSQL database"
    )]
    pub db_host: String,

    /// Name of the PostgreSQL database (default: khm)
    #[arg(
        long,
        default_value = "khm",
        help = "Server mode: Name of the PostgreSQL database"
    )]
    pub db_name: String,

    /// Username for the PostgreSQL database (required in server mode)
    #[arg(
        long,
        required_if_eq("server", "true"),
        help = "Server mode: Username for the PostgreSQL database"
    )]
    pub db_user: Option<String>,

    /// Password for the PostgreSQL database (required in server mode)
    #[arg(
        long,
        required_if_eq("server", "true"),
        help = "Server mode: Password for the PostgreSQL database"
    )]
    pub db_password: Option<String>,

    /// Host address of the server to connect to in client mode (required in client mode)
    #[arg(
        long,
        required_if_eq("server", "false"),
        help = "Client mode: Full host address of the server to connect to. Like https://khm.example.com"
    )]
    pub host: Option<String>,

    /// Flow name to use on the server
    #[arg(
        long,
        required_if_eq("server", "false"),
        help = "Client mode: Flow name to use on the server"
    )]
    pub flow: Option<String>,

    /// Path to the known_hosts file (default: ~/.ssh/known_hosts)
    #[arg(
        long,
        default_value = "~/.ssh/known_hosts",
        help = "Client mode: Path to the known_hosts file"
    )]
    pub known_hosts: String,

    /// Basic auth string for client mode. Format: user:pass
    #[arg(long, default_value = "", help = "Client mode: Basic Auth credentials")]
    pub basic_auth: String,
}

// Re-export WASM functions for wasm-pack
#[cfg(all(target_arch = "wasm32", feature = "web-gui"))]
pub use web_gui::wasm::*;