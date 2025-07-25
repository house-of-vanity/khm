use actix_web::{web, HttpResponse, Result};
use futures::future;
use log::info;
use rust_embed::RustEmbed;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use tokio::sync::Semaphore;
use tokio::time::{timeout, Duration};
use trust_dns_resolver::config::*;
use trust_dns_resolver::TokioAsyncResolver;

use crate::db::ReconnectingDbClient;
use crate::server::Flows;

#[derive(RustEmbed)]
#[folder = "static/"]
struct StaticAssets;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DnsResolutionResult {
    pub server: String,
    pub resolved: bool,
    pub error: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BulkDeprecateRequest {
    pub servers: Vec<String>,
}

async fn check_dns_resolution(hostname: String, semaphore: Arc<Semaphore>) -> DnsResolutionResult {
    let _permit = match semaphore.acquire().await {
        Ok(permit) => permit,
        Err(_) => {
            return DnsResolutionResult {
                server: hostname,
                resolved: false,
                error: Some("Failed to acquire semaphore".to_string()),
            };
        }
    };

    let resolver = TokioAsyncResolver::tokio(ResolverConfig::default(), ResolverOpts::default());

    let lookup_result = timeout(Duration::from_secs(5), resolver.lookup_ip(&hostname)).await;

    match lookup_result {
        Ok(Ok(_)) => DnsResolutionResult {
            server: hostname,
            resolved: true,
            error: None,
        },
        Ok(Err(e)) => DnsResolutionResult {
            server: hostname,
            resolved: false,
            error: Some(e.to_string()),
        },
        Err(_) => DnsResolutionResult {
            server: hostname,
            resolved: false,
            error: Some("DNS lookup timeout (5s)".to_string()),
        },
    }
}

// API endpoint to get application version
pub async fn get_version_api() -> Result<HttpResponse> {
    Ok(HttpResponse::Ok().json(json!({
        "version": env!("CARGO_PKG_VERSION")
    })))
}

// API endpoint to get list of available flows
pub async fn get_flows_api(allowed_flows: web::Data<Vec<String>>) -> Result<HttpResponse> {
    info!("API request for available flows");
    Ok(HttpResponse::Ok().json(&**allowed_flows))
}

// API endpoint to scan DNS resolution for all hosts in a flow
pub async fn scan_dns_resolution(
    flows: web::Data<Flows>,
    path: web::Path<String>,
    allowed_flows: web::Data<Vec<String>>,
) -> Result<HttpResponse> {
    let flow_id_str = path.into_inner();

    info!(
        "API request to scan DNS resolution for flow '{}'",
        flow_id_str
    );

    if !allowed_flows.contains(&flow_id_str) {
        return Ok(HttpResponse::Forbidden().json(json!({
            "error": "Flow ID not allowed"
        })));
    }

    let flows_guard = flows.lock().unwrap();
    let flow = match flows_guard.iter().find(|flow| flow.name == flow_id_str) {
        Some(flow) => flow,
        None => {
            return Ok(HttpResponse::NotFound().json(json!({
                "error": "Flow ID not found"
            })));
        }
    };

    // Get unique hostnames
    let mut hostnames: std::collections::HashSet<String> = std::collections::HashSet::new();
    for key in &flow.servers {
        hostnames.insert(key.server.clone());
    }

    drop(flows_guard);

    info!(
        "Scanning DNS resolution for {} unique hosts",
        hostnames.len()
    );

    // Limit concurrent DNS requests to prevent "too many open files" error
    let semaphore = Arc::new(Semaphore::new(20));

    // Scan all hostnames concurrently with rate limiting
    let mut scan_futures = Vec::new();
    for hostname in hostnames {
        scan_futures.push(check_dns_resolution(hostname, semaphore.clone()));
    }

    let results = future::join_all(scan_futures).await;

    let unresolved_count = results.iter().filter(|r| !r.resolved).count();
    info!(
        "DNS scan complete: {} unresolved out of {} hosts",
        unresolved_count,
        results.len()
    );

    Ok(HttpResponse::Ok().json(json!({
        "results": results,
        "total": results.len(),
        "unresolved": unresolved_count
    })))
}

// API endpoint to bulk deprecate multiple servers
pub async fn bulk_deprecate_servers(
    flows: web::Data<Flows>,
    path: web::Path<String>,
    request: web::Json<BulkDeprecateRequest>,
    db_client: web::Data<Arc<ReconnectingDbClient>>,
    allowed_flows: web::Data<Vec<String>>,
) -> Result<HttpResponse> {
    let flow_id_str = path.into_inner();

    info!(
        "API request to bulk deprecate {} servers in flow '{}'",
        request.servers.len(),
        flow_id_str
    );

    if !allowed_flows.contains(&flow_id_str) {
        return Ok(HttpResponse::Forbidden().json(json!({
            "error": "Flow ID not allowed"
        })));
    }

    // Use single bulk operation instead of loop
    let total_deprecated = match db_client
        .bulk_deprecate_keys_by_servers_reconnecting(request.servers.clone(), flow_id_str.clone())
        .await
    {
        Ok(count) => {
            info!(
                "Bulk deprecated {} key(s) for {} servers",
                count,
                request.servers.len()
            );
            count
        }
        Err(e) => {
            return Ok(HttpResponse::InternalServerError().json(json!({
                "error": format!("Failed to bulk deprecate keys: {}", e)
            })));
        }
    };

    // Refresh the in-memory flows
    let updated_flows = match db_client.get_keys_from_db_reconnecting().await {
        Ok(flows) => flows,
        Err(e) => {
            return Ok(HttpResponse::InternalServerError().json(json!({
                "error": format!("Failed to refresh flows: {}", e)
            })));
        }
    };

    let mut flows_guard = flows.lock().unwrap();
    *flows_guard = updated_flows;

    let response = json!({
        "message": format!("Successfully deprecated {} key(s) for {} server(s)", total_deprecated, request.servers.len()),
        "deprecated_count": total_deprecated,
        "servers_processed": request.servers.len()
    });

    Ok(HttpResponse::Ok().json(response))
}

// API endpoint to bulk restore multiple servers
pub async fn bulk_restore_servers(
    flows: web::Data<Flows>,
    path: web::Path<String>,
    request: web::Json<BulkDeprecateRequest>,
    db_client: web::Data<Arc<ReconnectingDbClient>>,
    allowed_flows: web::Data<Vec<String>>,
) -> Result<HttpResponse> {
    let flow_id_str = path.into_inner();

    info!(
        "API request to bulk restore {} servers in flow '{}'",
        request.servers.len(),
        flow_id_str
    );

    if !allowed_flows.contains(&flow_id_str) {
        return Ok(HttpResponse::Forbidden().json(json!({
            "error": "Flow ID not allowed"
        })));
    }

    // Use single bulk operation
    let total_restored = match db_client
        .bulk_restore_keys_by_servers_reconnecting(request.servers.clone(), flow_id_str.clone())
        .await
    {
        Ok(count) => {
            info!(
                "Bulk restored {} key(s) for {} servers",
                count,
                request.servers.len()
            );
            count
        }
        Err(e) => {
            return Ok(HttpResponse::InternalServerError().json(json!({
                "error": format!("Failed to bulk restore keys: {}", e)
            })));
        }
    };

    // Refresh the in-memory flows
    let updated_flows = match db_client.get_keys_from_db_reconnecting().await {
        Ok(flows) => flows,
        Err(e) => {
            return Ok(HttpResponse::InternalServerError().json(json!({
                "error": format!("Failed to refresh flows: {}", e)
            })));
        }
    };

    let mut flows_guard = flows.lock().unwrap();
    *flows_guard = updated_flows;

    let response = json!({
        "message": format!("Successfully restored {} key(s) for {} server(s)", total_restored, request.servers.len()),
        "restored_count": total_restored,
        "servers_processed": request.servers.len()
    });

    Ok(HttpResponse::Ok().json(response))
}

// API endpoint to deprecate a specific key by server name
pub async fn delete_key_by_server(
    flows: web::Data<Flows>,
    path: web::Path<(String, String)>,
    db_client: web::Data<Arc<ReconnectingDbClient>>,
    allowed_flows: web::Data<Vec<String>>,
) -> Result<HttpResponse> {
    let (flow_id_str, server_name) = path.into_inner();

    info!(
        "API request to deprecate key for server '{}' in flow '{}'",
        server_name, flow_id_str
    );

    if !allowed_flows.contains(&flow_id_str) {
        return Ok(HttpResponse::Forbidden().json(json!({
            "error": "Flow ID not allowed"
        })));
    }

    // Deprecate in database
    match db_client
        .deprecate_key_by_server_reconnecting(server_name.clone(), flow_id_str.clone())
        .await
    {
        Ok(deprecated_count) => {
            if deprecated_count > 0 {
                info!(
                    "Deprecated {} key(s) for server '{}' in flow '{}'",
                    deprecated_count, server_name, flow_id_str
                );

                // Refresh the in-memory flows
                let updated_flows = match db_client.get_keys_from_db_reconnecting().await {
                    Ok(flows) => flows,
                    Err(e) => {
                        return Ok(HttpResponse::InternalServerError().json(json!({
                            "error": format!("Failed to refresh flows: {}", e)
                        })));
                    }
                };

                let mut flows_guard = flows.lock().unwrap();
                *flows_guard = updated_flows;

                Ok(HttpResponse::Ok().json(json!({
                    "message": format!("Successfully deprecated {} key(s) for server '{}'", deprecated_count, server_name),
                    "deprecated_count": deprecated_count
                })))
            } else {
                Ok(HttpResponse::NotFound().json(json!({
                    "error": format!("No keys found for server '{}'", server_name)
                })))
            }
        }
        Err(e) => Ok(HttpResponse::InternalServerError().json(json!({
            "error": format!("Failed to deprecate key: {}", e)
        }))),
    }
}

// API endpoint to restore a deprecated key
pub async fn restore_key_by_server(
    flows: web::Data<Flows>,
    path: web::Path<(String, String)>,
    db_client: web::Data<Arc<ReconnectingDbClient>>,
    allowed_flows: web::Data<Vec<String>>,
) -> Result<HttpResponse> {
    let (flow_id_str, server_name) = path.into_inner();

    info!(
        "API request to restore key for server '{}' in flow '{}'",
        server_name, flow_id_str
    );

    if !allowed_flows.contains(&flow_id_str) {
        return Ok(HttpResponse::Forbidden().json(json!({
            "error": "Flow ID not allowed"
        })));
    }

    // Restore in database
    match db_client
        .restore_key_by_server_reconnecting(server_name.clone(), flow_id_str.clone())
        .await
    {
        Ok(restored_count) => {
            if restored_count > 0 {
                info!(
                    "Restored {} key(s) for server '{}' in flow '{}'",
                    restored_count, server_name, flow_id_str
                );

                // Refresh the in-memory flows
                let updated_flows = match db_client.get_keys_from_db_reconnecting().await {
                    Ok(flows) => flows,
                    Err(e) => {
                        return Ok(HttpResponse::InternalServerError().json(json!({
                            "error": format!("Failed to refresh flows: {}", e)
                        })));
                    }
                };

                let mut flows_guard = flows.lock().unwrap();
                *flows_guard = updated_flows;

                Ok(HttpResponse::Ok().json(json!({
                    "message": format!("Successfully restored {} key(s) for server '{}'", restored_count, server_name),
                    "restored_count": restored_count
                })))
            } else {
                Ok(HttpResponse::NotFound().json(json!({
                    "error": format!("No deprecated keys found for server '{}'", server_name)
                })))
            }
        }
        Err(e) => Ok(HttpResponse::InternalServerError().json(json!({
            "error": format!("Failed to restore key: {}", e)
        }))),
    }
}

// API endpoint to permanently delete a key
pub async fn permanently_delete_key_by_server(
    flows: web::Data<Flows>,
    path: web::Path<(String, String)>,
    db_client: web::Data<Arc<ReconnectingDbClient>>,
    allowed_flows: web::Data<Vec<String>>,
) -> Result<HttpResponse> {
    let (flow_id_str, server_name) = path.into_inner();

    info!(
        "API request to permanently delete key for server '{}' in flow '{}'",
        server_name, flow_id_str
    );

    if !allowed_flows.contains(&flow_id_str) {
        return Ok(HttpResponse::Forbidden().json(json!({
            "error": "Flow ID not allowed"
        })));
    }

    // Permanently delete from database
    match db_client
        .permanently_delete_key_by_server_reconnecting(server_name.clone(), flow_id_str.clone())
        .await
    {
        Ok(deleted_count) => {
            if deleted_count > 0 {
                info!(
                    "Permanently deleted {} key(s) for server '{}' in flow '{}'",
                    deleted_count, server_name, flow_id_str
                );

                // Refresh the in-memory flows
                let updated_flows = match db_client.get_keys_from_db_reconnecting().await {
                    Ok(flows) => flows,
                    Err(e) => {
                        return Ok(HttpResponse::InternalServerError().json(json!({
                            "error": format!("Failed to refresh flows: {}", e)
                        })));
                    }
                };

                let mut flows_guard = flows.lock().unwrap();
                *flows_guard = updated_flows;

                Ok(HttpResponse::Ok().json(json!({
                    "message": format!("Successfully deleted {} key(s) for server '{}'", deleted_count, server_name),
                    "deleted_count": deleted_count
                })))
            } else {
                Ok(HttpResponse::NotFound().json(json!({
                    "error": format!("No keys found for server '{}'", server_name)
                })))
            }
        }
        Err(e) => Ok(HttpResponse::InternalServerError().json(json!({
            "error": format!("Failed to delete key: {}", e)
        }))),
    }
}

// Serve static files from embedded assets
pub async fn serve_static_file(path: web::Path<String>) -> Result<HttpResponse> {
    let file_path = path.into_inner();

    match StaticAssets::get(&file_path) {
        Some(content) => {
            let content_type = match std::path::Path::new(&file_path)
                .extension()
                .and_then(|s| s.to_str())
            {
                Some("html") => "text/html; charset=utf-8",
                Some("css") => "text/css; charset=utf-8",
                Some("js") => "application/javascript; charset=utf-8",
                Some("png") => "image/png",
                Some("jpg") | Some("jpeg") => "image/jpeg",
                Some("svg") => "image/svg+xml",
                _ => "application/octet-stream",
            };

            Ok(HttpResponse::Ok()
                .content_type(content_type)
                .body(content.data.as_ref().to_vec()))
        }
        None => Ok(HttpResponse::NotFound().body(format!("File not found: {}", file_path))),
    }
}

// Serve the main web interface from embedded assets
pub async fn serve_web_interface() -> Result<HttpResponse> {
    match StaticAssets::get("index.html") {
        Some(content) => Ok(HttpResponse::Ok()
            .content_type("text/html; charset=utf-8")
            .body(content.data.as_ref().to_vec())),
        None => Ok(HttpResponse::NotFound().body("Web interface not found")),
    }
}
