# KHM - Known Hosts Manager

KHM is a comprehensive SSH key management tool that allows you to synchronize `known_hosts` files across multiple hosts and environments. The application supports multiple operation modes: server mode for centralized key storage, client mode for synchronization, and GUI mode for easy management.

## Features

- **Multi-mode operation**: Server, client, and GUI modes
- **Centralized key management**: Store SSH keys and flows in PostgreSQL database
- **Cross-platform GUI**: Modern tray application with settings window
- **Automatic synchronization**: Keep `known_hosts` files updated across environments
- **Flow-based organization**: Manage different environments (production, staging, development)
- **Authentication support**: Basic authentication for secure API access
- **Real-time monitoring**: Auto-sync capabilities with configurable intervals

## Operation Modes

### Server Mode
Runs a web server that stores and manages SSH keys in a PostgreSQL database.

```bash
khm --server --ip 0.0.0.0 --port 1337 --db-host psql.psql.svc --db-name khm --db-user admin --db-password <SECRET> --flows work,home
```

### Client Mode
Connects to a KHM server to send local keys and optionally sync the `known_hosts` file.

```bash
khm --host https://khm.example.com --flow work --known-hosts ~/.ssh/known_hosts --in-place
```

### GUI Mode
Launches a system tray application with a modern interface for easy management.

```bash
# Run tray application
khm --gui

# Run settings window only
khm --settings-ui
```

## Command Line Arguments

### General Options
- `--server` - Run in server mode
- `--gui` - Run with GUI tray interface
- `--settings-ui` - Run settings UI window (used with --gui)

### Server Mode Options
- `-i, --ip <IP>` - IP address to bind the server to [default: 127.0.0.1]
- `-p, --port <PORT>` - Port to bind the server to [default: 8080]
- `--flows <FLOWS>` - Comma-separated list of flows to manage [default: default]
- `--db-host <DB_HOST>` - PostgreSQL database hostname [default: 127.0.0.1]
- `--db-name <DB_NAME>` - PostgreSQL database name [default: khm]
- `--db-user <DB_USER>` - PostgreSQL database username (required)
- `--db-password <DB_PASSWORD>` - PostgreSQL database password (required)

### Client Mode Options
- `--host <HOST>` - Server URL (e.g., https://khm.example.com) (required)
- `--flow <FLOW>` - Flow name to use on the server (required)
- `--known-hosts <PATH>` - Path to known_hosts file [default: ~/.ssh/known_hosts]
- `--in-place` - Update known_hosts file with server keys after sync
- `--basic-auth <CREDENTIALS>` - Basic authentication (format: user:pass)

## GUI Features

The GUI mode provides:

- **System Tray Integration**: Runs quietly in the system tray
- **Settings Management**: Easy configuration through modern UI
- **Connection Testing**: Built-in server connectivity testing
- **Manual Synchronization**: On-demand sync operations
- **Auto-sync Configuration**: Configurable automatic synchronization intervals
- **Operation Logging**: Real-time activity monitoring
- **Cross-platform Paths**: Automatic path handling for different operating systems

## Installation

### From Binary Releases
Download the latest binary from the [Releases](https://github.com/house-of-vanity/khm/releases) page.

### From Source
1. Install Rust from [rustup.rs](https://rustup.rs/)
2. Clone the repository:
   ```bash
   git clone https://github.com/house-of-vanity/khm.git
   cd khm
   ```
3. Build and run:
   ```bash
   # Build with GUI support (default)
   cargo build --release
   
   # Build server-only version
   cargo build --release --no-default-features --features server
   ```

### System Dependencies
For GUI features on Linux:
```bash
sudo apt-get install libgtk-3-dev libglib2.0-dev libcairo2-dev libpango1.0-dev libatk1.0-dev libgdk-pixbuf2.0-dev
```

## Configuration

### GUI Configuration
Settings are automatically saved to:
- **Windows**: `%USERPROFILE%\.khm\khm_config.json`
- **macOS**: `~/.khm/khm_config.json`
- **Linux**: `~/.khm/khm_config.json`

### Example Configuration
```json
{
  "host": "https://khm.example.com",
  "flow": "production",
  "known_hosts": "/home/user/.ssh/known_hosts",
  "basic_auth": "",
  "in_place": true,
  "auto_sync_interval_minutes": 60
}
```

## Examples

### Complete Server Setup
```bash
# Start server with multiple flows
khm --server \
    --ip 0.0.0.0 \
    --port 8080 \
    --db-host localhost \
    --db-name khm \
    --db-user khm_user \
    --db-password secure_password \
    --flows production,staging,development
```

### Client Synchronization
```bash
# Send keys and update local known_hosts
khm --host https://khm.company.com \
    --flow production \
    --known-hosts ~/.ssh/known_hosts \
    --in-place \
    --basic-auth "username:password"
```

### GUI Usage
```bash
# Launch tray application
khm --gui

# Open settings window directly
khm --settings-ui
```

## Contributing

Contributions are welcome! Please feel free to submit issues, feature requests, or pull requests.

## License

This project is licensed under the WTFPL License - see the [LICENSE](LICENSE) file for details.