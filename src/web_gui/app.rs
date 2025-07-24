use super::state::{AdminSettings, AdminState, ConnectionStatus, AdminOperation};
use super::ui::{self, ConnectionAction, KeyAction, BulkAction};
#[cfg(not(target_arch = "wasm32"))]
use super::api;
#[cfg(target_arch = "wasm32")]
use super::wasm_api as api;
use eframe::egui;
use std::sync::mpsc;

pub struct WebAdminApp {
    settings: AdminSettings,
    admin_state: AdminState,
    flows: Vec<String>,
    connection_status: ConnectionStatus,
    operation_receiver: Option<mpsc::Receiver<AdminOperation>>,
    last_operation: String,
    server_version: Option<String>,
}

impl Default for WebAdminApp {
    fn default() -> Self {
        // Get server URL from current location if possible
        let server_url = {
            #[cfg(all(target_arch = "wasm32", feature = "web-gui"))]
            {
                web_sys::window()
                    .and_then(|w| w.location().origin().ok())
                    .unwrap_or_else(|| "http://localhost:8080".to_string())
            }
            #[cfg(not(all(target_arch = "wasm32", feature = "web-gui")))]
            {
                "http://localhost:8080".to_string()
            }
        };
        
        Self {
            settings: AdminSettings {
                server_url,
                ..Default::default()
            },
            admin_state: AdminState::default(),
            flows: Vec::new(),
            connection_status: ConnectionStatus::Disconnected,
            operation_receiver: None,
            last_operation: "Application started".to_string(),
            server_version: None,
        }
    }
}

impl eframe::App for WebAdminApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Handle async operations
        if let Some(receiver) = &self.operation_receiver {
            if let Ok(operation) = receiver.try_recv() {
                self.handle_operation_result(operation);
                ctx.request_repaint();
            }
        }
        
        // Use the same UI structure as desktop version
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("🔑 KHM Web Admin Panel");
            ui.separator();
            
            // Connection Settings (always visible for web version)
            egui::CollapsingHeader::new("⚙️ Connection Settings")
                .default_open(matches!(self.connection_status, ConnectionStatus::Disconnected))
                .show(ui, |ui| {
                    let connection_action = ui::render_connection_settings(
                        ui,
                        &mut self.settings,
                        &self.connection_status,
                        &self.flows,
                        &self.server_version,
                    );
                    
                    match connection_action {
                        ConnectionAction::LoadFlows => self.load_flows(ctx),
                        ConnectionAction::TestConnection => self.test_connection(ctx),
                        ConnectionAction::LoadKeys => self.load_keys(ctx),
                        ConnectionAction::LoadVersion => self.load_version(ctx),
                        ConnectionAction::None => {}
                    }
                });
            
            ui.add_space(10.0);
            
            // Statistics (from desktop version)
            if !self.admin_state.keys.is_empty() {
                ui::render_statistics(ui, &self.admin_state);
                ui.add_space(10.0);
            }
            
            // Key Management (from desktop version)
            if !self.admin_state.keys.is_empty() {
                egui::CollapsingHeader::new("🔑 Key Management")
                    .default_open(true)
                    .show(ui, |ui| {
                        // Search and filter controls (from desktop version)
                        ui::render_search_controls(ui, &mut self.admin_state);
                        ui.add_space(5.0);
                        
                        // Bulk actions (from desktop version)
                        let bulk_action = ui::render_bulk_actions(ui, &mut self.admin_state);
                        match bulk_action {
                            BulkAction::DeprecateSelected => self.bulk_deprecate(ctx),
                            BulkAction::RestoreSelected => self.bulk_restore(ctx),
                            BulkAction::ClearSelection => {
                                self.admin_state.clear_selection();
                            }
                            BulkAction::None => {}
                        }
                        
                        ui.add_space(5.0);
                        
                        // Keys table (from desktop version)
                        let key_action = ui::render_keys_table(ui, &mut self.admin_state);
                        match key_action {
                            KeyAction::DeprecateKey(server) => self.deprecate_key(server, ctx),
                            KeyAction::RestoreKey(server) => self.restore_key(server, ctx),
                            KeyAction::DeleteKey(server) => self.delete_key(server, ctx),
                            KeyAction::DeprecateServer(server) => self.deprecate_server(server, ctx),
                            KeyAction::RestoreServer(server) => self.restore_server(server, ctx),
                            KeyAction::None => {}
                        }
                    });
                
                ui.add_space(10.0);
            }
            
            // Additional web-specific actions
            if matches!(self.connection_status, ConnectionStatus::Connected) && !self.settings.selected_flow.is_empty() {
                ui.horizontal(|ui| {
                    if ui.button("🔍 Scan DNS").clicked() {
                        self.scan_dns(ctx);
                    }
                    
                    if ui.button("🔄 Refresh Keys").clicked() {
                        self.load_keys(ctx);
                    }
                    
                    ui.checkbox(&mut self.settings.auto_refresh, "Auto-refresh");
                });
                
                ui.add_space(10.0);
            }
            
            // Status bar (from desktop version)
            ui.horizontal(|ui| {
                ui.label("Status:");
                match &self.connection_status {
                    ConnectionStatus::Connected => {
                        ui.colored_label(egui::Color32::GREEN, "● Connected");
                    }
                    ConnectionStatus::Connecting => {
                        ui.colored_label(egui::Color32::YELLOW, "● Connecting...");
                    }
                    ConnectionStatus::Disconnected => {
                        ui.colored_label(egui::Color32::GRAY, "● Disconnected");
                    }
                    ConnectionStatus::Error(msg) => {
                        ui.colored_label(egui::Color32::RED, format!("● Error: {}", msg));
                    }
                }
                
                ui.separator();
                ui.label(&self.last_operation);
            });
        });
        
        // Auto-refresh like desktop version
        if self.settings.auto_refresh && matches!(self.connection_status, ConnectionStatus::Connected) {
            ctx.request_repaint_after(std::time::Duration::from_secs(self.settings.refresh_interval as u64));
        }
    }
}

impl WebAdminApp {
    fn handle_operation_result(&mut self, operation: AdminOperation) {
        match operation {
            AdminOperation::LoadFlows(result) => {
                match result {
                    Ok(flows) => {
                        self.flows = flows;
                        if !self.flows.is_empty() && self.settings.selected_flow.is_empty() {
                            self.settings.selected_flow = self.flows[0].clone();
                        }
                        self.last_operation = format!("Loaded {} flows", self.flows.len());
                    }
                    Err(err) => {
                        self.connection_status = ConnectionStatus::Error(err.clone());
                        self.last_operation = format!("Failed to load flows: {}", err);
                    }
                }
            }
            AdminOperation::LoadKeys(result) => {
                match result {
                    Ok(keys) => {
                        self.admin_state.keys = keys;
                        self.admin_state.filter_keys();
                        self.connection_status = ConnectionStatus::Connected;
                        self.last_operation = format!("Loaded {} keys", self.admin_state.keys.len());
                    }
                    Err(err) => {
                        self.connection_status = ConnectionStatus::Error(err.clone());
                        self.last_operation = format!("Failed to load keys: {}", err);
                    }
                }
            }
            AdminOperation::TestConnection(result) => {
                match result {
                    Ok(msg) => {
                        self.connection_status = ConnectionStatus::Connected;
                        self.last_operation = msg;
                    }
                    Err(err) => {
                        self.connection_status = ConnectionStatus::Error(err.clone());
                        self.last_operation = format!("Connection failed: {}", err);
                    }
                }
            }
            AdminOperation::DeprecateKey(server, result) => {
                match result {
                    Ok(msg) => {
                        self.last_operation = msg;
                        self.load_keys_silent();
                    }
                    Err(err) => {
                        self.last_operation = format!("Failed to deprecate key for {}: {}", server, err);
                    }
                }
            }
            AdminOperation::RestoreKey(server, result) => {
                match result {
                    Ok(msg) => {
                        self.last_operation = msg;
                        self.load_keys_silent();
                    }
                    Err(err) => {
                        self.last_operation = format!("Failed to restore key for {}: {}", server, err);
                    }
                }
            }
            AdminOperation::DeleteKey(server, result) => {
                match result {
                    Ok(msg) => {
                        self.last_operation = msg;
                        self.load_keys_silent();
                    }
                    Err(err) => {
                        self.last_operation = format!("Failed to delete key for {}: {}", server, err);
                    }
                }
            }
            AdminOperation::BulkDeprecate(result) | AdminOperation::BulkRestore(result) => {
                match result {
                    Ok(msg) => {
                        self.last_operation = msg;
                        self.admin_state.clear_selection();
                        self.load_keys_silent();
                    }
                    Err(err) => {
                        self.last_operation = format!("Bulk operation failed: {}", err);
                    }
                }
            }
            AdminOperation::ScanDns(result) => {
                match result {
                    Ok(results) => {
                        let resolved = results.iter().filter(|r| r.resolved).count();
                        let total = results.len();
                        self.last_operation = format!("DNS scan completed: {}/{} servers resolved", resolved, total);
                    }
                    Err(err) => {
                        self.last_operation = format!("DNS scan failed: {}", err);
                    }
                }
            }
            AdminOperation::LoadVersion(result) => {
                match result {
                    Ok(version) => {
                        self.server_version = Some(version.clone());
                        self.last_operation = format!("Server version: {}", version);
                    }
                    Err(err) => {
                        self.last_operation = format!("Failed to get server version: {}", err);
                    }
                }
            }
        }
    }
    
    // Async operation methods - adapted from desktop version
    fn load_flows(&mut self, _ctx: &egui::Context) {
        self.last_operation = "Loading flows...".to_string();
        
        let settings = self.settings.clone();
        let (tx, rx) = mpsc::channel();
        self.operation_receiver = Some(rx);
        
        #[cfg(not(target_arch = "wasm32"))]
        {
            std::thread::spawn(move || {
                let rt = tokio::runtime::Runtime::new().unwrap();
                let result = rt.block_on(api::load_flows(&settings));
                let _ = tx.send(AdminOperation::LoadFlows(result));
            });
        }
        
        #[cfg(all(target_arch = "wasm32", feature = "web-gui"))]
        {
            wasm_bindgen_futures::spawn_local(async move {
                let result = api::load_flows(&settings).await;
                let _ = tx.send(AdminOperation::LoadFlows(result));
            });
        }
    }
    
    fn test_connection(&mut self, _ctx: &egui::Context) {
        self.connection_status = ConnectionStatus::Connecting;
        self.last_operation = "Testing connection...".to_string();
        
        let settings = self.settings.clone();
        let (tx, rx) = mpsc::channel();
        self.operation_receiver = Some(rx);
        
        #[cfg(not(target_arch = "wasm32"))]
        {
            std::thread::spawn(move || {
                let rt = tokio::runtime::Runtime::new().unwrap();
                let result = rt.block_on(api::test_connection(&settings));
                let _ = tx.send(AdminOperation::TestConnection(result));
            });
        }
        
        #[cfg(all(target_arch = "wasm32", feature = "web-gui"))]
        {
            wasm_bindgen_futures::spawn_local(async move {
                let result = api::test_connection(&settings).await;
                let _ = tx.send(AdminOperation::TestConnection(result));
            });
        }
    }
    
    fn load_keys(&mut self, _ctx: &egui::Context) {
        self.admin_state.current_operation = "Loading keys...".to_string();
        self.last_operation = "Loading keys...".to_string();
        
        let settings = self.settings.clone();
        let (tx, rx) = mpsc::channel();
        self.operation_receiver = Some(rx);
        
        #[cfg(not(target_arch = "wasm32"))]
        {
            std::thread::spawn(move || {
                let rt = tokio::runtime::Runtime::new().unwrap();
                let result = rt.block_on(api::fetch_keys(&settings));
                let _ = tx.send(AdminOperation::LoadKeys(result));
            });
        }
        
        #[cfg(all(target_arch = "wasm32", feature = "web-gui"))]
        {
            wasm_bindgen_futures::spawn_local(async move {
                let result = api::fetch_keys(&settings).await;
                let _ = tx.send(AdminOperation::LoadKeys(result));
            });
        }
    }
    
    fn load_keys_silent(&mut self) {
        let settings = self.settings.clone();
        let (tx, rx) = mpsc::channel();
        self.operation_receiver = Some(rx);
        
        #[cfg(not(target_arch = "wasm32"))]
        {
            std::thread::spawn(move || {
                let rt = tokio::runtime::Runtime::new().unwrap();
                let result = rt.block_on(api::fetch_keys(&settings));
                let _ = tx.send(AdminOperation::LoadKeys(result));
            });
        }
        
        #[cfg(all(target_arch = "wasm32", feature = "web-gui"))]
        {
            wasm_bindgen_futures::spawn_local(async move {
                let result = api::fetch_keys(&settings).await;
                let _ = tx.send(AdminOperation::LoadKeys(result));
            });
        }
    }
    
    fn deprecate_key(&mut self, server: String, _ctx: &egui::Context) {
        self.last_operation = format!("Deprecating key for {}...", server);
        
        let settings = self.settings.clone();
        let server_clone = server.clone();
        let (tx, rx) = mpsc::channel();
        self.operation_receiver = Some(rx);
        
        #[cfg(not(target_arch = "wasm32"))]
        {
            std::thread::spawn(move || {
                let rt = tokio::runtime::Runtime::new().unwrap();
                let result = rt.block_on(api::deprecate_key(&settings, &server));
                let _ = tx.send(AdminOperation::DeprecateKey(server_clone, result));
            });
        }
        
        #[cfg(all(target_arch = "wasm32", feature = "web-gui"))]
        {
            wasm_bindgen_futures::spawn_local(async move {
                let result = api::deprecate_key(&settings, &server).await;
                let _ = tx.send(AdminOperation::DeprecateKey(server_clone, result));
            });
        }
    }
    
    fn restore_key(&mut self, server: String, _ctx: &egui::Context) {
        self.last_operation = format!("Restoring key for {}...", server);
        
        let settings = self.settings.clone();
        let server_clone = server.clone();
        let (tx, rx) = mpsc::channel();
        self.operation_receiver = Some(rx);
        
        #[cfg(not(target_arch = "wasm32"))]
        {
            std::thread::spawn(move || {
                let rt = tokio::runtime::Runtime::new().unwrap();
                let result = rt.block_on(api::restore_key(&settings, &server));
                let _ = tx.send(AdminOperation::RestoreKey(server_clone, result));
            });
        }
        
        #[cfg(all(target_arch = "wasm32", feature = "web-gui"))]
        {
            wasm_bindgen_futures::spawn_local(async move {
                let result = api::restore_key(&settings, &server).await;
                let _ = tx.send(AdminOperation::RestoreKey(server_clone, result));
            });
        }
    }
    
    fn delete_key(&mut self, server: String, _ctx: &egui::Context) {
        self.last_operation = format!("Deleting key for {}...", server);
        
        let settings = self.settings.clone();
        let server_clone = server.clone();
        let (tx, rx) = mpsc::channel();
        self.operation_receiver = Some(rx);
        
        #[cfg(not(target_arch = "wasm32"))]
        {
            std::thread::spawn(move || {
                let rt = tokio::runtime::Runtime::new().unwrap();
                let result = rt.block_on(api::delete_key(&settings, &server));
                let _ = tx.send(AdminOperation::DeleteKey(server_clone, result));
            });
        }
        
        #[cfg(all(target_arch = "wasm32", feature = "web-gui"))]
        {
            wasm_bindgen_futures::spawn_local(async move {
                let result = api::delete_key(&settings, &server).await;
                let _ = tx.send(AdminOperation::DeleteKey(server_clone, result));
            });
        }
    }
    
    fn deprecate_server(&mut self, server: String, ctx: &egui::Context) {
        self.deprecate_key(server, ctx);
    }
    
    fn restore_server(&mut self, server: String, ctx: &egui::Context) {
        self.restore_key(server, ctx);
    }
    
    fn bulk_deprecate(&mut self, _ctx: &egui::Context) {
        let servers = self.admin_state.get_selected_servers();
        if servers.is_empty() {
            return;
        }
        
        self.last_operation = format!("Bulk deprecating {} servers...", servers.len());
        
        let settings = self.settings.clone();
        let (tx, rx) = mpsc::channel();
        self.operation_receiver = Some(rx);
        
        #[cfg(not(target_arch = "wasm32"))]
        {
            std::thread::spawn(move || {
                let rt = tokio::runtime::Runtime::new().unwrap();
                let result = rt.block_on(api::bulk_deprecate_servers(&settings, servers));
                let _ = tx.send(AdminOperation::BulkDeprecate(result));
            });
        }
        
        #[cfg(all(target_arch = "wasm32", feature = "web-gui"))]
        {
            wasm_bindgen_futures::spawn_local(async move {
                let result = api::bulk_deprecate_servers(&settings, servers).await;
                let _ = tx.send(AdminOperation::BulkDeprecate(result));
            });
        }
    }
    
    fn bulk_restore(&mut self, _ctx: &egui::Context) {
        let servers = self.admin_state.get_selected_servers();
        if servers.is_empty() {
            return;
        }
        
        self.last_operation = format!("Bulk restoring {} servers...", servers.len());
        
        let settings = self.settings.clone();
        let (tx, rx) = mpsc::channel();
        self.operation_receiver = Some(rx);
        
        #[cfg(not(target_arch = "wasm32"))]
        {
            std::thread::spawn(move || {
                let rt = tokio::runtime::Runtime::new().unwrap();
                let result = rt.block_on(api::bulk_restore_servers(&settings, servers));
                let _ = tx.send(AdminOperation::BulkRestore(result));
            });
        }
        
        #[cfg(all(target_arch = "wasm32", feature = "web-gui"))]
        {
            wasm_bindgen_futures::spawn_local(async move {
                let result = api::bulk_restore_servers(&settings, servers).await;
                let _ = tx.send(AdminOperation::BulkRestore(result));
            });
        }
    }
    
    fn scan_dns(&mut self, _ctx: &egui::Context) {
        self.last_operation = "Scanning DNS resolution...".to_string();
        
        let settings = self.settings.clone();
        let (tx, rx) = mpsc::channel();
        self.operation_receiver = Some(rx);
        
        #[cfg(not(target_arch = "wasm32"))]
        {
            std::thread::spawn(move || {
                let rt = tokio::runtime::Runtime::new().unwrap();
                let result = rt.block_on(api::scan_dns_resolution(&settings));
                let _ = tx.send(AdminOperation::ScanDns(result));
            });
        }
        
        #[cfg(all(target_arch = "wasm32", feature = "web-gui"))]
        {
            wasm_bindgen_futures::spawn_local(async move {
                let result = api::scan_dns_resolution(&settings).await;
                let _ = tx.send(AdminOperation::ScanDns(result));
            });
        }
    }
    
    fn load_version(&mut self, _ctx: &egui::Context) {
        self.last_operation = "Loading server version...".to_string();
        
        let settings = self.settings.clone();
        let (tx, rx) = mpsc::channel();
        self.operation_receiver = Some(rx);
        
        #[cfg(not(target_arch = "wasm32"))]
        {
            std::thread::spawn(move || {
                let rt = tokio::runtime::Runtime::new().unwrap();
                let result = rt.block_on(api::get_version(&settings));
                let _ = tx.send(AdminOperation::LoadVersion(result));
            });
        }
        
        #[cfg(all(target_arch = "wasm32", feature = "web-gui"))]
        {
            wasm_bindgen_futures::spawn_local(async move {
                let result = api::get_version(&settings).await;
                let _ = tx.send(AdminOperation::LoadVersion(result));
            });
        }
    }
}