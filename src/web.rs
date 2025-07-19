use actix_web::{web, HttpResponse, Result};
use log::info;
use rust_embed::RustEmbed;
use serde_json::json;
use std::sync::Arc;

use crate::db::ReconnectingDbClient;
use crate::server::Flows;

#[derive(RustEmbed)]
#[folder = "static/"]
struct StaticAssets;

// API endpoint to get list of available flows
pub async fn get_flows_api(allowed_flows: web::Data<Vec<String>>) -> Result<HttpResponse> {
    info!("API request for available flows");
    Ok(HttpResponse::Ok().json(&**allowed_flows))
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
