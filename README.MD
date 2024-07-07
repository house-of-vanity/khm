# KHM - Known Hosts Manager

KHM allows you to synchronize the `known_hosts` file across multiple hosts. This application manages SSH keys and flows, either as a server or client. In server mode, it stores keys and flows in a PostgreSQL database. In client mode, it sends keys to the server and can update the `known_hosts` file with keys from the server.

## Features

- Synchronize `known_hosts` file across multiple hosts.
- Manage SSH keys and flows in a PostgreSQL database.
- Operate in both server and client modes.
- Automatically update `known_hosts` file with keys from the server.

## Usage

### Server Mode

To run the application in server mode, use the following command:

```bash
khm --server --ip 127.0.0.1 --port 8080 --db-host 127.0.0.1 --db-name khm --db-user admin --db-password <SECRET> --flows work,home
```

### Client Mode

To run the application in client mode, use the following command:

```bash
khm --host http://khm.example.com:8080 --known-hosts ~/.ssh/known_hosts --in-place
```

### Arguments
- `--server`: Run in server mode (default: false).
- `--ip`: IP address to bind the server or client to (default: 127.0.0.1).
- `--port`: Port to bind the server or client to (default: 8080).
- `--db-host`: Hostname or IP address of the PostgreSQL database (default: 127.0.0.1).
- `--db-name`: Name of the PostgreSQL database (default: khm).
- `--db-user`: Username for the PostgreSQL database (required in server mode).
- `--db-password`: Password for the PostgreSQL database (required in server mode).
- `--host`: Host address of the server to connect to in client mode (required in client mode).
- `--known-hosts`: Path to the `known_hosts` file (default: ~/.ssh/known_hosts).
- `--in-place`: Update the `known_hosts` file with keys from the server after sending keys (default: false).
- `--flows`: Comma-separated list of flows to manage (default: default).

## Installation

1. Ensure you have Rust installed. If not, you can install it from [rustup.rs](https://rustup.rs/).
2. Clone the repository:
    ```bash
    git clone https://github.com/house-of-vanity/khm.git
    cd khm
    ```
3. Run the project:
    ```bash
    cargo run --release -- --help
    ```

## Contributing

Contributions are welcome! Please open an issue or submit a pull request for any changes.

## License

This project is licensed under the WTFPL License.