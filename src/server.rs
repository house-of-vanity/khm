use actix_web::{web, App, HttpRequest, HttpResponse, HttpServer, Responder};
use log::{error, info};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

use crate::db::ReconnectingDbClient;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SshKey {
    pub server: String,
    pub public_key: String,
    #[serde(default)]
    pub deprecated: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Flow {
    pub name: String,
    pub servers: Vec<SshKey>,
}

pub type Flows = Arc<Mutex<Vec<Flow>>>;

pub fn is_valid_ssh_key(key: &str) -> bool {
    let rsa_re = Regex::new(r"^ssh-rsa AAAA[0-9A-Za-z+/]+[=]{0,3}( .+)?$").unwrap();
    let dsa_re = Regex::new(r"^ssh-dss AAAA[0-9A-Za-z+/]+[=]{0,3}( .+)?$").unwrap();
    let ecdsa_re =
        Regex::new(r"^ecdsa-sha2-nistp(256|384|521) AAAA[0-9A-Za-z+/]+[=]{0,3}( .+)?$").unwrap();
    let ed25519_re = Regex::new(r"^ssh-ed25519 AAAA[0-9A-Za-z+/]+[=]{0,3}( .+)?$").unwrap();

    rsa_re.is_match(key)
        || dsa_re.is_match(key)
        || ecdsa_re.is_match(key)
        || ed25519_re.is_match(key)
}

// Extract client hostname from request headers
fn get_client_hostname(req: &HttpRequest) -> String {
    if let Some(hostname) = req.headers().get("X-Client-Hostname") {
        if let Ok(hostname_str) = hostname.to_str() {
            return hostname_str.to_string();
        }
    }
    "unknown-client".to_string()
}

pub async fn get_keys(
    flows: web::Data<Flows>,
    flow_id: web::Path<String>,
    allowed_flows: web::Data<Vec<String>>,
    req: HttpRequest,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> impl Responder {
    let client_hostname = get_client_hostname(&req);
    let flow_id_str = flow_id.into_inner();

    info!(
        "Received keys request from client '{}' for flow '{}'",
        client_hostname, flow_id_str
    );

    if !allowed_flows.contains(&flow_id_str) {
        error!(
            "Flow ID not allowed for client '{}': {}",
            client_hostname, flow_id_str
        );
        return HttpResponse::Forbidden().body("Flow ID not allowed");
    }

    let flows = flows.lock().unwrap();
    if let Some(flow) = flows.iter().find(|flow| flow.name == flow_id_str) {
        // Check if we should include deprecated keys (default: false for CLI clients)
        let include_deprecated = query
            .get("include_deprecated")
            .map(|v| v == "true")
            .unwrap_or(false);

        let servers: Vec<&SshKey> = if include_deprecated {
            // Return all keys (for web interface)
            flow.servers.iter().collect()
        } else {
            // Return only active keys (for CLI clients)
            flow.servers.iter().filter(|key| !key.deprecated).collect()
        };

        info!(
            "Returning {} keys ({} total, deprecated filtered: {}) for flow '{}' to client '{}'",
            servers.len(),
            flow.servers.len(),
            !include_deprecated,
            flow_id_str,
            client_hostname
        );
        HttpResponse::Ok().json(servers)
    } else {
        error!(
            "Flow ID not found for client '{}': {}",
            client_hostname, flow_id_str
        );
        HttpResponse::NotFound().body("Flow ID not found")
    }
}

pub async fn add_keys(
    flows: web::Data<Flows>,
    flow_id: web::Path<String>,
    new_keys: web::Json<Vec<SshKey>>,
    db_client: web::Data<Arc<ReconnectingDbClient>>,
    allowed_flows: web::Data<Vec<String>>,
    req: HttpRequest,
) -> impl Responder {
    let client_hostname = get_client_hostname(&req);
    let flow_id_str = flow_id.into_inner();

    info!(
        "Received {} keys from client '{}' for flow '{}'",
        new_keys.len(),
        client_hostname,
        flow_id_str
    );

    if !allowed_flows.contains(&flow_id_str) {
        error!(
            "Flow ID not allowed for client '{}': {}",
            client_hostname, flow_id_str
        );
        return HttpResponse::Forbidden().body("Flow ID not allowed");
    }

    // Check SSH key format
    let mut valid_keys = Vec::new();
    for new_key in new_keys.iter() {
        if !is_valid_ssh_key(&new_key.public_key) {
            error!(
                "Invalid SSH key format from client '{}' for server: {}",
                client_hostname, new_key.server
            );
            return HttpResponse::BadRequest().body(format!(
                "Invalid SSH key format for server: {}",
                new_key.server
            ));
        }
        valid_keys.push(new_key.clone());
    }

    info!(
        "Processing batch of {} keys from client '{}' for flow: {}",
        valid_keys.len(),
        client_hostname,
        flow_id_str
    );

    // Batch insert keys with statistics
    let key_stats = match db_client
        .batch_insert_keys_reconnecting(valid_keys.clone())
        .await
    {
        Ok(stats) => stats,
        Err(e) => {
            error!(
                "Failed to batch insert keys from client '{}' into database: {}",
                client_hostname, e
            );
            return HttpResponse::InternalServerError()
                .body("Failed to batch insert keys into database");
        }
    };

    // Always try to associate all keys with the flow, regardless of whether they're new or existing
    if !key_stats.key_id_map.is_empty() {
        // Extract all key IDs from statistics, both new and existing
        let key_ids: Vec<i32> = key_stats.key_id_map.iter().map(|(_, id)| *id).collect();

        // Batch insert key-flow associations
        if let Err(e) = db_client
            .batch_insert_flow_keys_reconnecting(flow_id_str.clone(), key_ids.clone())
            .await
        {
            error!(
                "Failed to batch insert flow keys from client '{}' into database: {}",
                client_hostname, e
            );
            return HttpResponse::InternalServerError()
                .body("Failed to batch insert flow keys into database");
        }

        info!(
            "Added flow associations for {} keys from client '{}' in flow '{}'",
            key_ids.len(),
            client_hostname,
            flow_id_str
        );
    } else {
        info!(
            "No keys to associate from client '{}' with flow '{}'",
            client_hostname, flow_id_str
        );
    }

    // Get updated data
    let updated_flows = match db_client.get_keys_from_db_reconnecting().await {
        Ok(flows) => flows,
        Err(e) => {
            error!(
                "Failed to get updated flows from database after client '{}' request: {}",
                client_hostname, e
            );
            return HttpResponse::InternalServerError()
                .body("Failed to refresh flows from database");
        }
    };

    let mut flows_guard = flows.lock().unwrap();
    *flows_guard = updated_flows;

    let updated_flow = flows_guard.iter().find(|flow| flow.name == flow_id_str);
    if let Some(flow) = updated_flow {
        let servers: Vec<&SshKey> = flow.servers.iter().collect();
        info!(
            "Keys summary for client '{}', flow '{}': total received={}, new={}, unchanged={}, total in flow={}",
            client_hostname,
            flow_id_str,
            key_stats.total,
            key_stats.inserted,
            key_stats.unchanged,
            servers.len()
        );

        // Add statistics to HTTP response headers
        let mut response = HttpResponse::Ok();
        response.append_header(("X-Keys-Total", key_stats.total.to_string()));
        response.append_header(("X-Keys-New", key_stats.inserted.to_string()));
        response.append_header(("X-Keys-Unchanged", key_stats.unchanged.to_string()));

        response.json(servers)
    } else {
        error!(
            "Flow ID not found after update from client '{}': {}",
            client_hostname, flow_id_str
        );
        HttpResponse::NotFound().body("Flow ID not found")
    }
}

pub async fn run_server(args: crate::Args) -> std::io::Result<()> {
    let db_user = args.db_user.expect("db_user is required in server mode");
    let db_password = args
        .db_password
        .expect("db_password is required in server mode");

    let db_conn_str = format!(
        "host={} user={} password={} dbname={}",
        args.db_host, db_user, db_password, args.db_name
    );

    info!("Creating database client for {}", args.db_host);
    let mut db_client_temp = ReconnectingDbClient::new(db_conn_str.clone());

    // Initial connection
    if let Err(e) = db_client_temp.connect(&db_conn_str).await {
        error!("Failed to connect to the database: {}", e);
        return Err(std::io::Error::new(
            std::io::ErrorKind::ConnectionRefused,
            format!("Database connection error: {}", e),
        ));
    }

    let db_client = Arc::new(db_client_temp);

    // Initialize database schema if needed
    if let Err(e) = db_client.initialize_schema().await {
        error!("Failed to initialize database schema: {}", e);
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("Database schema initialization error: {}", e),
        ));
    }

    let mut initial_flows = match db_client.get_keys_from_db_reconnecting().await {
        Ok(flows) => flows,
        Err(e) => {
            error!("Failed to get initial flows from database: {}", e);
            Vec::new()
        }
    };

    // Ensure all allowed flows are initialized
    for allowed_flow in &args.flows {
        if !initial_flows.iter().any(|flow| &flow.name == allowed_flow) {
            initial_flows.push(Flow {
                name: allowed_flow.clone(),
                servers: vec![],
            });
        }
    }

    let flows: Flows = Arc::new(Mutex::new(initial_flows));
    let allowed_flows = web::Data::new(args.flows);

    info!("Starting HTTP server on {}:{}", args.ip, args.port);
    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(flows.clone()))
            .app_data(web::Data::new(db_client.clone()))
            .app_data(allowed_flows.clone())
            // API routes
            .route("/api/version", web::get().to(crate::web::get_version_api))
            .route("/api/flows", web::get().to(crate::web::get_flows_api))
            .route(
                "/{flow_id}/scan-dns",
                web::post().to(crate::web::scan_dns_resolution),
            )
            .route(
                "/{flow_id}/bulk-deprecate",
                web::post().to(crate::web::bulk_deprecate_servers),
            )
            .route(
                "/{flow_id}/bulk-restore",
                web::post().to(crate::web::bulk_restore_servers),
            )
            .route(
                "/{flow_id}/keys/{server}",
                web::delete().to(crate::web::delete_key_by_server),
            )
            .route(
                "/{flow_id}/keys/{server}/restore",
                web::post().to(crate::web::restore_key_by_server),
            )
            .route(
                "/{flow_id}/keys/{server}/delete",
                web::delete().to(crate::web::permanently_delete_key_by_server),
            )
            // Original API routes
            .route("/{flow_id}/keys", web::get().to(get_keys))
            .route("/{flow_id}/keys", web::post().to(add_keys))
            // Web interface routes
            .route("/", web::get().to(crate::web::serve_web_interface))
            .route(
                "/static/{filename:.*}",
                web::get().to(crate::web::serve_static_file),
            )
    })
    .bind((args.ip.as_str(), args.port))?
    .run()
    .await
}
