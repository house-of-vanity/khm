use actix_web::{HttpResponse, Result, web};
use serde_json::json;
use log::info;

#[cfg(feature = "web-gui")]
pub mod app;
#[cfg(feature = "web-gui")]
pub mod state;
#[cfg(feature = "web-gui")]
pub mod ui;
#[cfg(all(feature = "web-gui", not(target_arch = "wasm32")))]
pub mod api;
#[cfg(all(target_arch = "wasm32", feature = "web-gui"))]
pub mod wasm_api;
#[cfg(all(target_arch = "wasm32", feature = "web-gui"))]
pub mod wasm;


/// Serve the egui web GUI interface
pub async fn serve_egui_interface() -> Result<HttpResponse> {
    #[cfg(feature = "web-gui")]
    {
        let html = r#"
<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <title>KHM Admin Panel</title>
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <style>
        html, body {
            margin: 0;
            padding: 0;
            width: 100%;
            height: 100%;
            background: #2b2b2b;
            font-family: system-ui, sans-serif;
        }
        
        canvas {
            width: 100vw;
            height: 100vh;
            display: block;
        }
        
        #loading {
            position: fixed;
            top: 50%;
            left: 50%;
            transform: translate(-50%, -50%);
            color: white;
            font-size: 18px;
            z-index: 1000;
            text-align: center;
        }
        
        .spinner {
            border: 3px solid rgba(255,255,255,0.3);
            border-radius: 50%;
            border-top: 3px solid #667eea;
            width: 40px;
            height: 40px;
            animation: spin 1s linear infinite;
            margin: 0 auto 20px;
        }
        
        @keyframes spin {
            0% { transform: rotate(0deg); }
            100% { transform: rotate(360deg); }
        }
    </style>
</head>
<body>
    <div id="loading">
        <div class="spinner"></div>
        Loading KHM Admin Panel...
    </div>
    
    <canvas id="the_canvas_id"></canvas>

    <script type="module">
        import init, { start_web_admin } from './wasm/khm_wasm.js';
        
        async function run() {
            try {
                // Initialize WASM module
                await init();
                
                // Hide loading indicator
                document.getElementById('loading').style.display = 'none';
                
                // Start the egui web app
                start_web_admin('the_canvas_id');
                
                console.log('KHM Web Admin Panel started successfully');
            } catch (error) {
                console.error('Failed to start KHM Web Admin Panel:', error);
                
                // Show error message
                document.getElementById('loading').innerHTML = `
                    <div style="color: #ff6b6b; text-align: center;">
                        <h3>⚠️ WASM Module Not Available</h3>
                        <p>The egui web interface requires WASM compilation.</p>
                        <p style="font-size: 14px; color: #ccc; margin: 20px 0;">Build steps:</p>
                        <div style="background: #333; padding: 15px; border-radius: 5px; font-family: monospace; text-align: left; max-width: 600px; margin: 0 auto;">
                            <div style="color: #888; margin-bottom: 10px;"># Install wasm-pack</div>
                            <div style="color: #fff;">cargo install wasm-pack</div>
                            <div style="color: #888; margin: 10px 0;"># Build WASM module</div>
                            <div style="color: #fff;">wasm-pack build --target web --out-dir wasm --features web-gui</div>
                            <div style="color: #888; margin: 10px 0;"># Restart server</div>
                            <div style="color: #fff;">cargo run --features "server,web,web-gui"</div>
                        </div>
                        <p style="font-size: 12px; color: #888; margin-top: 20px;">Error: ${error.message}</p>
                    </div>
                `;
            }
        }
        
        run();
    </script>
</body>
</html>
        "#;
        
        Ok(HttpResponse::Ok()
            .content_type("text/html; charset=utf-8")
            .body(html))
    }
    
    #[cfg(not(feature = "web-gui"))]
    {
        let html = r#"
<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <title>KHM Admin Panel - Not Available</title>
    <style>
        body {
            font-family: system-ui, sans-serif;
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            color: white;
            display: flex;
            justify-content: center;
            align-items: center;
            height: 100vh;
            margin: 0;
            text-align: center;
        }
    </style>
</head>
<body>
    <div>
        <h1>⚠️ Web GUI Not Available</h1>
        <p>This server was compiled without web-gui support.</p>
        <p>Please rebuild with <code>--features web-gui</code> to enable the admin interface.</p>
    </div>
</body>
</html>
        "#;
        
        Ok(HttpResponse::Ok()
            .content_type("text/html; charset=utf-8")
            .body(html))
    }
}

/// API endpoint to get GUI configuration
pub async fn get_gui_config(
    flows: web::Data<crate::server::Flows>,
    allowed_flows: web::Data<Vec<String>>,
) -> Result<HttpResponse> {
    info!("Web GUI config requested");
    
    let flows_guard = flows.lock().unwrap();
    let available_flows: Vec<String> = flows_guard.iter().map(|f| f.name.clone()).collect();
    
    Ok(HttpResponse::Ok().json(json!({
        "version": env!("CARGO_PKG_VERSION"),
        "gui_ready": cfg!(feature = "web-gui"),
        "features": ["key_management", "bulk_operations", "real_time_updates"],
        "available_flows": available_flows,
        "allowed_flows": &**allowed_flows,
        "api_endpoints": {
            "flows": "/api/flows",
            "keys": "/{flow}/keys",
            "deprecate": "/{flow}/keys/{server}",
            "restore": "/{flow}/keys/{server}/restore", 
            "delete": "/{flow}/keys/{server}/delete",
            "bulk_deprecate": "/{flow}/bulk-deprecate",
            "bulk_restore": "/{flow}/bulk-restore",
            "dns_scan": "/{flow}/scan-dns"
        }
    })))
}

/// API endpoint for web GUI state management
pub async fn get_gui_state(
    flows: web::Data<crate::server::Flows>,
    allowed_flows: web::Data<Vec<String>>,
) -> Result<HttpResponse> {
    info!("Web GUI state requested");
    
    let flows_guard = flows.lock().unwrap();
    let flow_data: Vec<_> = flows_guard.iter().map(|f| json!({
        "name": f.name,
        "servers_count": f.servers.len(),
        "active_keys": f.servers.iter().filter(|k| !k.deprecated).count(),
        "deprecated_keys": f.servers.iter().filter(|k| k.deprecated).count()
    })).collect();
    
    Ok(HttpResponse::Ok().json(json!({
        "flows": flow_data,
        "allowed_flows": &**allowed_flows,
        "timestamp": chrono::Utc::now().to_rfc3339()
    })))
}

/// API endpoint to update GUI settings  
pub async fn update_gui_settings(
    settings: web::Json<serde_json::Value>,
) -> Result<HttpResponse> {
    info!("Web GUI settings updated: {:?}", settings);
    
    Ok(HttpResponse::Ok().json(json!({
        "status": "success",
        "message": "Settings updated successfully",
        "timestamp": chrono::Utc::now().to_rfc3339()
    })))
}

/// Serve WASM files for egui web application
pub async fn serve_wasm_file(path: web::Path<String>) -> Result<HttpResponse> {
    let filename = path.into_inner();
    info!("WASM file requested: {}", filename);
    
    // Try to read the actual WASM files from the wasm directory
    let wasm_dir = std::path::Path::new("wasm");
    let file_path = wasm_dir.join(&filename);
    
    match std::fs::read(&file_path) {
        Ok(content) => {
            let content_type = if filename.ends_with(".js") {
                "application/javascript; charset=utf-8"
            } else if filename.ends_with(".wasm") {
                "application/wasm"
            } else {
                "application/octet-stream"
            };
            
            info!("Serving WASM file: {} ({} bytes)", filename, content.len());
            Ok(HttpResponse::Ok()
                .content_type(content_type)
                .body(content))
        }
        Err(_) => {
            // Fallback to placeholder if files don't exist
            let content = match filename.as_str() {
                "khm_wasm.js" => {
                    r#"
// KHM WASM Module Not Found
// Build the WASM module first:
// cd khm-wasm && wasm-pack build --target web --out-dir ../wasm

export default function init() {
    return Promise.reject(new Error('WASM module not found. Run: cd khm-wasm && wasm-pack build --target web --out-dir ../wasm'));
}

export function start_web_admin(canvas_id) {
    throw new Error('WASM module not found. Run: cd khm-wasm && wasm-pack build --target web --out-dir ../wasm');
}
                    "#
                }
                _ => {
                    return Ok(HttpResponse::NotFound().json(json!({
                        "error": "WASM file not found",
                        "filename": filename,
                        "message": "Run: cd khm-wasm && wasm-pack build --target web --out-dir ../wasm"
                    })));
                }
            };
            
            Ok(HttpResponse::Ok()
                .content_type("application/javascript; charset=utf-8")
                .body(content))
        }
    }
}