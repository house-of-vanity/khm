use actix_web::{web, HttpResponse, Result};
use log::info;
use rust_embed::RustEmbed;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashSet;
use tokio_postgres::Client;

use crate::server::{get_keys_from_db, Flows};

#[derive(RustEmbed)]
#[folder = "static/"]
struct StaticAssets;

#[derive(Deserialize)]
struct DeleteKeyPath {
    flow_id: String,
    server: String,
}

// API endpoint to get list of available flows
pub async fn get_flows_api(allowed_flows: web::Data<Vec<String>>) -> Result<HttpResponse> {
    info!("API request for available flows");
    Ok(HttpResponse::Ok().json(&**allowed_flows))
}

// API endpoint to delete a specific key by server name
pub async fn delete_key_by_server(
    flows: web::Data<Flows>,
    path: web::Path<(String, String)>,
    db_client: web::Data<std::sync::Arc<Client>>,
    allowed_flows: web::Data<Vec<String>>,
) -> Result<HttpResponse> {
    let (flow_id_str, server_name) = path.into_inner();

    info!("API request to delete key for server '{}' in flow '{}'", server_name, flow_id_str);

    if !allowed_flows.contains(&flow_id_str) {
        return Ok(HttpResponse::Forbidden().json(json!({
            "error": "Flow ID not allowed"
        })));
    }

    // Delete from database
    match delete_key_from_db(&db_client, &server_name, &flow_id_str).await {
        Ok(deleted_count) => {
            if deleted_count > 0 {
                info!("Deleted {} key(s) for server '{}' in flow '{}'", deleted_count, server_name, flow_id_str);
                
                // Refresh the in-memory flows
                let updated_flows = match get_keys_from_db(&db_client).await {
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
        Err(e) => {
            Ok(HttpResponse::InternalServerError().json(json!({
                "error": format!("Failed to delete key: {}", e)
            })))
        }
    }
}

// Helper function to delete a key from database
async fn delete_key_from_db(
    client: &Client,
    server_name: &str,
    flow_name: &str,
) -> Result<u64, tokio_postgres::Error> {
    // First, find the key_ids for the given server
    let key_rows = client
        .query("SELECT key_id FROM public.keys WHERE host = $1", &[&server_name])
        .await?;

    if key_rows.is_empty() {
        return Ok(0);
    }

    let key_ids: Vec<i32> = key_rows.iter().map(|row| row.get::<_, i32>(0)).collect();

    // Delete flow associations first
    let mut flow_delete_count = 0;
    for key_id in &key_ids {
        let deleted = client
            .execute(
                "DELETE FROM public.flows WHERE name = $1 AND key_id = $2",
                &[&flow_name, key_id],
            )
            .await?;
        flow_delete_count += deleted;
    }

    // Check if any of these keys are used in other flows
    let mut keys_to_delete = Vec::new();
    for key_id in &key_ids {
        let count: i64 = client
            .query_one(
                "SELECT COUNT(*) FROM public.flows WHERE key_id = $1",
                &[key_id],
            )
            .await?
            .get(0);

        if count == 0 {
            keys_to_delete.push(*key_id);
        }
    }

    // Delete keys that are no longer referenced by any flow
    let mut total_deleted = 0;
    for key_id in keys_to_delete {
        let deleted = client
            .execute("DELETE FROM public.keys WHERE key_id = $1", &[&key_id])
            .await?;
        total_deleted += deleted;
    }

    info!(
        "Deleted {} flow associations and {} orphaned keys for server '{}'",
        flow_delete_count, total_deleted, server_name
    );

    Ok(std::cmp::max(flow_delete_count, total_deleted))
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
        None => {
            Ok(HttpResponse::NotFound().body(format!("File not found: {}", file_path)))
        }
    }
}

// Serve the main web interface from embedded assets
pub async fn serve_web_interface() -> Result<HttpResponse> {
    match StaticAssets::get("index.html") {
        Some(content) => {
            Ok(HttpResponse::Ok()
                .content_type("text/html; charset=utf-8")
                .body(content.data.as_ref().to_vec()))
        }
        None => {
            Ok(HttpResponse::NotFound().body("Web interface not found"))
        }
    }
}
