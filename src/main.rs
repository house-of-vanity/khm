mod client;
mod db;
mod server;
mod web;

use clap::Parser;
use env_logger;
use log::{error, info};

/// This application manages SSH keys and flows, either as a server or client.
/// In server mode, it stores keys and flows in a PostgreSQL database.
/// In client mode, it sends keys to the server and can update the known_hosts file with keys from the server.
#[derive(Parser, Debug)]
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
    khm --host https://khm.example.com/work --known-hosts ~/.ssh/known_hosts --in-place\n\
    \n\
    "
)]
struct Args {
    /// Run in server mode (default: false)
    #[arg(long, help = "Run in server mode")]
    server: bool,

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
        help = "Client mode: Full host address of the server to connect to. Like https://khm.example.com/<FLOW_NAME>"
    )]
    host: Option<String>,

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
    env_logger::init();
    info!("Starting SSH Key Manager");

    let args = Args::parse();

    // Check if we have the minimum required arguments
    if !args.server && args.host.is_none() {
        // Neither server mode nor client mode properly configured
        eprintln!("Error: You must specify either server mode (--server) or client mode (--host)");
        eprintln!();
        eprintln!("Examples:");
        eprintln!(
            "  Server mode: {} --server --db-user admin --db-password pass --flows work,home",
            env!("CARGO_PKG_NAME")
        );
        eprintln!(
            "  Client mode: {} --host https://khm.example.com/work",
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
