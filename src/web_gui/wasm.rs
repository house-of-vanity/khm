#[cfg(all(target_arch = "wasm32", feature = "web-gui"))]
use wasm_bindgen::prelude::*;

#[cfg(all(target_arch = "wasm32", feature = "web-gui"))]
use super::app::WebAdminApp;

/// WASM entry point for the web admin application
#[cfg(all(target_arch = "wasm32", feature = "web-gui"))]
#[wasm_bindgen]
pub fn start_web_admin(canvas_id: &str) -> Result<(), JsValue> {
    // Setup console logging for WASM
    console_error_panic_hook::set_once();
    tracing_wasm::set_as_global_default();
    
    let web_options = eframe::WebOptions::default();
    let canvas_id = canvas_id.to_string();
    
    wasm_bindgen_futures::spawn_local(async move {
        let app = WebAdminApp::default();
        
        let result = eframe::WebRunner::new()
            .start(
                &canvas_id,
                web_options,
                Box::new(|_cc| Ok(Box::new(app))),
            )
            .await;
            
        match result {
            Ok(_) => web_sys::console::log_1(&"eframe started successfully".into()),
            Err(e) => web_sys::console::error_1(&format!("Failed to start eframe: {:?}", e).into()),
        }
    });
    
    Ok(())
}

/// Initialize the WASM module
#[cfg(all(target_arch = "wasm32", feature = "web-gui"))]
#[wasm_bindgen(start)]
pub fn wasm_main() {
    console_error_panic_hook::set_once();
}