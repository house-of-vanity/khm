use khm::{client, server, Args};

use clap::Parser;
use env_logger;
use log::{error, info};

/// CLI version of KHM - Known Hosts Manager for SSH key management and synchronization
/// Supports server and client modes without GUI dependencies
#[derive(Parser, Debug, Clone)]
#[command(
    author = env!("CARGO_PKG_AUTHORS"),
    version = env!("CARGO_PKG_VERSION"),
    about = "SSH Host Key Manager (CLI with Server)",
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
pub struct CliArgs {
    /// Run in server mode (default: false)
    #[arg(long, help = "Run in server mode")]
    pub server: bool,

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

impl From<CliArgs> for Args {
    fn from(cli_args: CliArgs) -> Self {
        Args {
            server: cli_args.server,
            daemon: false,
            settings_ui: false,
            in_place: cli_args.in_place,
            flows: cli_args.flows,
            ip: cli_args.ip,
            port: cli_args.port,
            db_host: cli_args.db_host,
            db_name: cli_args.db_name,
            db_user: cli_args.db_user,
            db_password: cli_args.db_password,
            host: cli_args.host,
            flow: cli_args.flow,
            known_hosts: cli_args.known_hosts,
            basic_auth: cli_args.basic_auth,
        }
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Configure logging to show only khm logs, filtering out noisy library logs
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Warn) // Default level for all modules
        .filter_module("khm", log::LevelFilter::Debug) // Our app logs
        .filter_module("actix_web", log::LevelFilter::Info) // Server logs
        .filter_module("reqwest", log::LevelFilter::Warn) // HTTP client
        .init();

    info!("Starting SSH Key Manager (CLI)");

    let cli_args = CliArgs::parse();
    let args: Args = cli_args.into();

    // Validate arguments - either server mode or client mode with required args
    if !args.server && (args.host.is_none() || args.flow.is_none()) {
        error!("CLI version requires either --server mode or client mode with --host and --flow arguments");
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Invalid arguments for CLI mode",
        ));
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