use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, BTreeMap, HashSet};
use std::future::Future;
use web_sys::{window, Request, RequestInit, RequestMode, Response};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SshKey {
    pub server: String,
    pub public_key: String,
    #[serde(default)]
    pub deprecated: bool,
}

#[derive(Debug, Clone)]
pub struct AdminSettings {
    pub selected_flow: String,
}

impl Default for AdminSettings {
    fn default() -> Self {
        Self {
            selected_flow: String::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct AdminState {
    pub keys: Vec<SshKey>,
    pub filtered_keys: Vec<SshKey>,
    pub search_term: String,
    pub show_deprecated_only: bool,
    pub show_active_only: bool,
    pub selected_servers: HashMap<String, bool>,
    pub expanded_servers: HashMap<String, bool>,
    pub current_operation: String,
}

#[derive(Debug, Clone)]
pub struct AdminStatistics {
    pub total_keys: usize,
    pub active_keys: usize,
    pub deprecated_keys: usize,
    pub unique_servers: usize,
}

#[derive(Debug, Clone)]
pub enum KeyAction {
    None,
    DeprecateKey(String),
    RestoreKey(String),
    DeleteKey(String),
    DeprecateServer(String),
    RestoreServer(String),
}

#[derive(Debug, Clone)]
pub enum BulkAction {
    None,
    DeprecateSelected,
    RestoreSelected,
    ClearSelection,
}

impl Default for AdminState {
    fn default() -> Self {
        Self {
            keys: Vec::new(),
            filtered_keys: Vec::new(),
            search_term: String::new(),
            show_deprecated_only: false,
            show_active_only: false,
            selected_servers: HashMap::new(),
            expanded_servers: HashMap::new(),
            current_operation: String::new(),
        }
    }
}

impl AdminState {
    pub fn filter_keys(&mut self) {
        let mut filtered = self.keys.clone();
        
        // Apply status filter
        if self.show_deprecated_only {
            filtered.retain(|key| key.deprecated);
        } else if self.show_active_only {
            filtered.retain(|key| !key.deprecated);
        }
        // By default, show all keys (both active and deprecated)
        
        // Apply search filter
        if !self.search_term.is_empty() {
            let search_term = self.search_term.to_lowercase();
            filtered.retain(|key| {
                key.server.to_lowercase().contains(&search_term)
                    || key.public_key.to_lowercase().contains(&search_term)
            });
        }
        
        self.filtered_keys = filtered;
    }
    
    pub fn get_statistics(&self) -> AdminStatistics {
        let total_keys = self.keys.len();
        let active_keys = self.keys.iter().filter(|k| !k.deprecated).count();
        let deprecated_keys = total_keys - active_keys;
        let unique_servers = self
            .keys
            .iter()
            .map(|k| &k.server)
            .collect::<HashSet<_>>()
            .len();
        
        AdminStatistics {
            total_keys,
            active_keys,
            deprecated_keys,
            unique_servers,
        }
    }
    
    pub fn get_selected_servers(&self) -> Vec<String> {
        self.selected_servers
            .iter()
            .filter_map(|(server, &selected)| {
                if selected { Some(server.clone()) } else { None }
            })
            .collect()
    }
    
    pub fn clear_selection(&mut self) {
        self.selected_servers.clear();
    }
}

// Utility functions
pub fn get_key_type(public_key: &str) -> String {
    if public_key.starts_with("ssh-rsa") {
        "RSA".to_string()
    } else if public_key.starts_with("ssh-ed25519") {
        "ED25519".to_string()
    } else if public_key.starts_with("ecdsa-sha2-nistp") {
        "ECDSA".to_string()
    } else if public_key.starts_with("ssh-dss") {
        "DSA".to_string()
    } else {
        "Unknown".to_string()
    }
}

pub fn get_key_preview(public_key: &str) -> String {
    let parts: Vec<&str> = public_key.split_whitespace().collect();
    if parts.len() >= 2 {
        let key_part = parts[1];
        if key_part.len() > 12 {
            format!("{}...", &key_part[..12])
        } else {
            key_part.to_string()
        }
    } else {
        format!("{}...", &public_key[..std::cmp::min(12, public_key.len())])
    }
}

// API functions for WASM
async fn fetch_api(url: &str) -> Result<Response, JsValue> {
    let mut opts = RequestInit::new();
    opts.method("GET");
    opts.mode(RequestMode::Cors);
    
    let request = Request::new_with_str_and_init(url, &opts)?;
    
    let window = window().unwrap();
    let resp_value = JsFuture::from(window.fetch_with_request(&request)).await?;
    let resp: Response = resp_value.dyn_into()?;
    
    Ok(resp)
}

async fn fetch_flows() -> Result<Vec<String>, JsValue> {
    let resp = fetch_api("/api/flows").await?;
    let json = JsFuture::from(resp.json()?).await?;
    let flows: Vec<String> = serde_wasm_bindgen::from_value(json)?;
    Ok(flows)
}

async fn fetch_keys(flow: &str) -> Result<Vec<SshKey>, JsValue> {
    let url = format!("/{}/keys", flow);
    let resp = fetch_api(&url).await?;
    let json = JsFuture::from(resp.json()?).await?;
    let keys: Vec<SshKey> = serde_wasm_bindgen::from_value(json)?;
    Ok(keys)
}

pub struct WebAdminApp {
    settings: AdminSettings,
    admin_state: AdminState,
    status_message: String,
    available_flows: Vec<String>,
    loading: bool,
    flows_promise: Option<wasm_bindgen_futures::JsFuture>,
    keys_promise: Option<wasm_bindgen_futures::JsFuture>,
    json_promise: Option<wasm_bindgen_futures::JsFuture>,
    operation_promise: Option<wasm_bindgen_futures::JsFuture>,
    pending_operation: String,
    flows_loaded: bool,
    auto_load_keys: bool,
}

impl Default for WebAdminApp {
    fn default() -> Self {
        Self {
            settings: AdminSettings::default(),
            admin_state: AdminState::default(),
            status_message: "Loading flows...".to_string(),
            available_flows: Vec::new(),
            loading: true,
            flows_promise: None,
            keys_promise: None,
            json_promise: None,
            operation_promise: None,
            pending_operation: String::new(),
            flows_loaded: false,
            auto_load_keys: false,
        }
    }
}

impl eframe::App for WebAdminApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Check if we're on mobile/small screen
        let screen_width = ctx.screen_rect().width();
        let is_mobile = screen_width < 600.0;
        
        // Set mobile-friendly spacing
        let base_spacing = if is_mobile { 8.0 } else { 10.0 };
        let button_height = if is_mobile { 44.0 } else { 32.0 }; // Touch-friendly size
        // Auto-load flows on startup
        if !self.flows_loaded && self.flows_promise.is_none() {
            self.load_flows();
        }
        
        // Auto-load keys when flow changes
        if self.auto_load_keys && !self.settings.selected_flow.is_empty() && self.keys_promise.is_none() && self.json_promise.is_none() {
            self.load_keys();
            self.auto_load_keys = false;
        }
        
        // Check for completed promises
        if let Some(mut promise) = self.flows_promise.take() {
            use std::task::{Context, Poll, Waker};
            use std::pin::Pin;
            
            struct DummyWaker;
            impl std::task::Wake for DummyWaker {
                fn wake(self: std::sync::Arc<Self>) {}
            }
            let waker = Waker::from(std::sync::Arc::new(DummyWaker));
            let mut cx = Context::from_waker(&waker);
            
            match Pin::new(&mut promise).poll(&mut cx) {
                Poll::Ready(Ok(response_js)) => {
                    if let Ok(response) = response_js.dyn_into::<web_sys::Response>() {
                        if response.ok() {
                            let json_promise = response.json().unwrap();
                            self.json_promise = Some(wasm_bindgen_futures::JsFuture::from(json_promise));
                            self.pending_operation = "flows".to_string();
                            self.status_message = "Parsing flows response...".to_string();
                        } else {
                            self.loading = false;
                            self.status_message = "Failed to load flows".to_string();
                        }
                    }
                }
                Poll::Ready(Err(_)) => {
                    self.loading = false;
                    self.status_message = "Error loading flows".to_string();
                }
                Poll::Pending => {
                    self.flows_promise = Some(promise);
                    ctx.request_repaint();
                }
            }
        }
        
        if let Some(mut promise) = self.keys_promise.take() {
            use std::task::{Context, Poll, Waker};
            use std::pin::Pin;
            
            struct DummyWaker;
            impl std::task::Wake for DummyWaker {
                fn wake(self: std::sync::Arc<Self>) {}
            }
            let waker = Waker::from(std::sync::Arc::new(DummyWaker));
            let mut cx = Context::from_waker(&waker);
            
            match Pin::new(&mut promise).poll(&mut cx) {
                Poll::Ready(Ok(response_js)) => {
                    if let Ok(response) = response_js.dyn_into::<web_sys::Response>() {
                        if response.ok() {
                            let json_promise = response.json().unwrap();
                            self.json_promise = Some(wasm_bindgen_futures::JsFuture::from(json_promise));
                            self.pending_operation = "keys".to_string();
                            self.status_message = "Parsing keys response...".to_string();
                        } else {
                            self.loading = false;
                            self.status_message = "Failed to load keys".to_string();
                        }
                    }
                }
                Poll::Ready(Err(_)) => {
                    self.loading = false;
                    self.status_message = "Error loading keys".to_string();
                }
                Poll::Pending => {
                    self.keys_promise = Some(promise);
                    ctx.request_repaint();
                }
            }
        }
        
        // Check for completed operations
        if let Some(mut promise) = self.operation_promise.take() {
            use std::task::{Context, Poll, Waker};
            use std::pin::Pin;
            
            struct DummyWaker;
            impl std::task::Wake for DummyWaker {
                fn wake(self: std::sync::Arc<Self>) {}
            }
            let waker = Waker::from(std::sync::Arc::new(DummyWaker));
            let mut cx = Context::from_waker(&waker);
            
            match Pin::new(&mut promise).poll(&mut cx) {
                Poll::Ready(Ok(response_js)) => {
                    self.loading = false;
                    if let Ok(response) = response_js.dyn_into::<web_sys::Response>() {
                        if response.ok() {
                            let parts: Vec<&str> = self.pending_operation.split(':').collect();
                            if parts.len() == 2 {
                                let operation = parts[0];
                                let param = parts[1];
                                match operation {
                                    "deprecate" => {
                                        self.status_message = format!("Key deprecated for {}", param);
                                        self.load_keys(); // Reload to show changes
                                    }
                                    "restore" => {
                                        self.status_message = format!("Key restored for {}", param);
                                        self.load_keys(); // Reload to show changes
                                    }
                                    "delete" => {
                                        self.status_message = format!("Key deleted for {}", param);
                                        self.load_keys(); // Reload to show changes
                                    }
                                    "bulk-deprecate" => {
                                        self.status_message = format!("Deprecated {} servers", param);
                                        self.admin_state.clear_selection(); // Clear selection after bulk operation
                                        self.load_keys(); // Reload to show changes
                                    }
                                    "bulk-restore" => {
                                        self.status_message = format!("Restored {} servers", param);
                                        self.admin_state.clear_selection(); // Clear selection after bulk operation
                                        self.load_keys(); // Reload to show changes
                                    }
                                    _ => {
                                        self.status_message = "Operation completed".to_string();
                                    }
                                }
                            }
                        } else {
                            self.status_message = "Operation failed".to_string();
                        }
                    }
                    self.pending_operation.clear();
                }
                Poll::Ready(Err(_)) => {
                    self.loading = false;
                    self.status_message = "Operation error".to_string();
                    self.pending_operation.clear();
                }
                Poll::Pending => {
                    self.operation_promise = Some(promise);
                    ctx.request_repaint();
                }
            }
        }
        
        // Check for completed JSON parsing
        if let Some(mut promise) = self.json_promise.take() {
            use std::task::{Context, Poll, Waker};
            use std::pin::Pin;
            
            struct DummyWaker;
            impl std::task::Wake for DummyWaker {
                fn wake(self: std::sync::Arc<Self>) {}
            }
            let waker = Waker::from(std::sync::Arc::new(DummyWaker));
            let mut cx = Context::from_waker(&waker);
            
            match Pin::new(&mut promise).poll(&mut cx) {
                Poll::Ready(Ok(json_data)) => {
                    self.loading = false;
                    
                    match self.pending_operation.as_str() {
                        "flows" => {
                            if let Ok(flows) = serde_wasm_bindgen::from_value::<Vec<String>>(json_data) {
                                self.available_flows = flows.clone();
                                self.flows_loaded = true;
                                
                                // Auto-select first flow
                                if !flows.is_empty() && self.settings.selected_flow.is_empty() {
                                    self.settings.selected_flow = flows[0].clone();
                                    self.auto_load_keys = true;
                                }
                                
                                self.status_message = format!("Loaded {} flows", flows.len());
                            } else {
                                self.status_message = "Failed to parse flows data".to_string();
                            }
                        }
                        "keys" => {
                            if let Ok(keys) = serde_wasm_bindgen::from_value::<Vec<SshKey>>(json_data) {
                                self.admin_state.keys = keys.clone();
                                self.admin_state.filter_keys();
                                self.status_message = format!("Loaded {} keys", keys.len());
                            } else {
                                self.status_message = "Failed to parse keys data".to_string();
                            }
                        }
                        _ => {
                            self.status_message = "Unknown operation completed".to_string();
                        }
                    }
                    
                    self.pending_operation.clear();
                }
                Poll::Ready(Err(_)) => {
                    self.loading = false;
                    self.status_message = "Error parsing JSON response".to_string();
                    self.pending_operation.clear();
                }
                Poll::Pending => {
                    self.json_promise = Some(promise);
                    ctx.request_repaint();
                }
            }
        }
        
        egui::CentralPanel::default().show(ctx, |ui| {
            let title_size = if is_mobile { 22.0 } else { 28.0 };
            ui.add_space(base_spacing);
            ui.heading(egui::RichText::new("🔑 KHM Web Admin Panel").size(title_size));
            ui.separator();
            ui.add_space(base_spacing * 1.5);
            
            // Flow Selection
            ui.group(|ui| {
                ui.set_min_width(ui.available_width());
                ui.vertical(|ui| {
                    let section_title_size = if is_mobile { 16.0 } else { 18.0 };
                    ui.label(egui::RichText::new("📂 Flow Selection").size(section_title_size).strong());
                    ui.add_space(base_spacing);
                    
                    // Use vertical layout on mobile for better space usage
                    if is_mobile {
                        ui.vertical(|ui| {
                            ui.label(egui::RichText::new("Current Flow:").size(14.0));
                            ui.add_space(5.0);
                            
                            let mut flow_changed = false;
                            let old_flow = self.settings.selected_flow.clone();
                            
                            egui::ComboBox::from_id_salt("flow_selector")
                                .selected_text(if self.settings.selected_flow.is_empty() { "Select flow..." } else { &self.settings.selected_flow })
                                .width(ui.available_width() - 20.0)
                                .show_ui(ui, |ui| {
                                    for flow in &self.available_flows {
                                        if ui.selectable_value(&mut self.settings.selected_flow, flow.clone(), egui::RichText::new(flow).size(14.0)).clicked() {
                                            flow_changed = true;
                                        }
                                    }
                                });
                            
                            if flow_changed && old_flow != self.settings.selected_flow {
                                self.auto_load_keys = true;
                            }
                            
                            ui.add_space(base_spacing);
                            
                            if ui.add_sized([ui.available_width(), button_height], egui::Button::new(egui::RichText::new("🔄 Refresh").size(14.0))).clicked() && !self.loading {
                                if !self.settings.selected_flow.is_empty() {
                                    self.load_keys();
                                }
                            }
                        });
                    } else {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("Current Flow:").size(16.0));
                            ui.add_space(10.0);
                            
                            let mut flow_changed = false;
                            let old_flow = self.settings.selected_flow.clone();
                            
                            egui::ComboBox::from_id_salt("flow_selector")
                                .selected_text(if self.settings.selected_flow.is_empty() { "Select flow..." } else { &self.settings.selected_flow })
                                .width(300.0)
                                .show_ui(ui, |ui| {
                                    for flow in &self.available_flows {
                                        if ui.selectable_value(&mut self.settings.selected_flow, flow.clone(), egui::RichText::new(flow).size(14.0)).clicked() {
                                            flow_changed = true;
                                        }
                                    }
                                });
                            
                            if flow_changed && old_flow != self.settings.selected_flow {
                                self.auto_load_keys = true;
                            }
                            
                            ui.add_space(20.0);
                            
                            if ui.add_sized([120.0, button_height], egui::Button::new(egui::RichText::new("🔄 Refresh").size(14.0))).clicked() && !self.loading {
                                if !self.settings.selected_flow.is_empty() {
                                    self.load_keys();
                                }
                            }
                        });
                    }
                });
            });
            
            ui.add_space(base_spacing);
            
            // Statistics
            if !self.admin_state.keys.is_empty() {
                self.render_statistics(ui, is_mobile);
                ui.add_space(base_spacing);
            }
            
            // Search and filters
            if !self.admin_state.keys.is_empty() {
                self.render_search_controls(ui, is_mobile);
                ui.add_space(base_spacing);
            }
            
            // Bulk actions
            let bulk_action = self.render_bulk_actions(ui, is_mobile, button_height);
            if bulk_action != BulkAction::None {
                self.handle_bulk_action(bulk_action);
            }
            
            // Keys display
            if !self.admin_state.filtered_keys.is_empty() {
                let key_action = self.render_keys_table(ui, is_mobile, button_height);
                if key_action != KeyAction::None {
                    self.handle_key_action(key_action);
                }
            } else if !self.admin_state.keys.is_empty() {
                self.render_empty_state(ui);
            }
            
            ui.add_space(10.0);
            
            // Status bar
            ui.separator();
            ui.horizontal(|ui| {
                ui.label("Status:");
                ui.colored_label(egui::Color32::LIGHT_BLUE, &self.status_message);
            });
        });
    }
}

impl WebAdminApp {
    fn load_flows(&mut self) {
        self.status_message = "Loading flows...".to_string();
        
        let window = web_sys::window().unwrap();
        let opts = web_sys::RequestInit::new();
        opts.set_method("GET");
        opts.set_mode(web_sys::RequestMode::Cors);
        
        if let Ok(request) = web_sys::Request::new_with_str_and_init("/api/flows", &opts) {
            let promise = window.fetch_with_request(&request);
            self.flows_promise = Some(wasm_bindgen_futures::JsFuture::from(promise));
            self.loading = true;
        }
    }
    
    fn load_keys(&mut self) {
        if self.settings.selected_flow.is_empty() {
            return;
        }
        
        self.status_message = format!("Loading keys for {}...", self.settings.selected_flow);
        
        // Add include_deprecated=true to show all keys (active and deprecated)
        let url = format!("/{}/keys?include_deprecated=true", self.settings.selected_flow);
        let window = web_sys::window().unwrap();
        let opts = web_sys::RequestInit::new();
        opts.set_method("GET");
        opts.set_mode(web_sys::RequestMode::Cors);
        
        if let Ok(request) = web_sys::Request::new_with_str_and_init(&url, &opts) {
            let promise = window.fetch_with_request(&request);
            self.keys_promise = Some(wasm_bindgen_futures::JsFuture::from(promise));
            self.loading = true;
        }
    }
    
    fn deprecate_key(&mut self, server: &str) {
        if self.settings.selected_flow.is_empty() {
            return;
        }
        
        self.status_message = format!("Deprecating key for {}...", server);
        
        let url = format!("/{}/keys/{}", self.settings.selected_flow, server);
        let window = web_sys::window().unwrap();
        let opts = web_sys::RequestInit::new();
        opts.set_method("DELETE");  // Правильный метод для deprecate
        opts.set_mode(web_sys::RequestMode::Cors);
        
        if let Ok(request) = web_sys::Request::new_with_str_and_init(&url, &opts) {
            let promise = window.fetch_with_request(&request);
            self.operation_promise = Some(wasm_bindgen_futures::JsFuture::from(promise));
            self.pending_operation = format!("deprecate:{}", server);
            self.loading = true;
        }
    }
    
    fn restore_key(&mut self, server: &str) {
        if self.settings.selected_flow.is_empty() {
            return;
        }
        
        self.status_message = format!("Restoring key for {}...", server);
        
        let url = format!("/{}/keys/{}/restore", self.settings.selected_flow, server);
        let window = web_sys::window().unwrap();
        let opts = web_sys::RequestInit::new();
        opts.set_method("POST");
        opts.set_mode(web_sys::RequestMode::Cors);
        
        if let Ok(request) = web_sys::Request::new_with_str_and_init(&url, &opts) {
            let promise = window.fetch_with_request(&request);
            self.operation_promise = Some(wasm_bindgen_futures::JsFuture::from(promise));
            self.pending_operation = format!("restore:{}", server);
            self.loading = true;
        }
    }
    
    fn delete_key(&mut self, server: &str) {
        if self.settings.selected_flow.is_empty() {
            return;
        }
        
        self.status_message = format!("Deleting key for {}...", server);
        
        let url = format!("/{}/keys/{}/delete", self.settings.selected_flow, server);
        let window = web_sys::window().unwrap();
        let opts = web_sys::RequestInit::new();
        opts.set_method("DELETE");  // Правильный метод для delete
        opts.set_mode(web_sys::RequestMode::Cors);
        
        if let Ok(request) = web_sys::Request::new_with_str_and_init(&url, &opts) {
            let promise = window.fetch_with_request(&request);
            self.operation_promise = Some(wasm_bindgen_futures::JsFuture::from(promise));
            self.pending_operation = format!("delete:{}", server);
            self.loading = true;
        }
    }
    
    fn bulk_deprecate_servers(&mut self, servers: Vec<String>) {
        if self.settings.selected_flow.is_empty() {
            return;
        }
        
        self.status_message = format!("Deprecating {} servers...", servers.len());
        
        let url = format!("/{}/bulk-deprecate", self.settings.selected_flow);
        let window = web_sys::window().unwrap();
        let opts = web_sys::RequestInit::new();
        opts.set_method("POST");
        opts.set_mode(web_sys::RequestMode::Cors);
        
        // Create JSON body
        let body = serde_json::json!({
            "servers": servers
        });
        
        if let Ok(body_str) = serde_json::to_string(&body) {
            opts.set_body(&wasm_bindgen::JsValue::from_str(&body_str));
            opts.set_headers(&{
                let headers = web_sys::Headers::new().unwrap();
                headers.set("Content-Type", "application/json").unwrap();
                headers.into()
            });
            
            if let Ok(request) = web_sys::Request::new_with_str_and_init(&url, &opts) {
                let promise = window.fetch_with_request(&request);
                self.operation_promise = Some(wasm_bindgen_futures::JsFuture::from(promise));
                self.pending_operation = format!("bulk-deprecate:{}", servers.len());
                self.loading = true;
            }
        }
    }
    
    fn bulk_restore_servers(&mut self, servers: Vec<String>) {
        if self.settings.selected_flow.is_empty() {
            return;
        }
        
        self.status_message = format!("Restoring {} servers...", servers.len());
        
        let url = format!("/{}/bulk-restore", self.settings.selected_flow);
        let window = web_sys::window().unwrap();
        let opts = web_sys::RequestInit::new();
        opts.set_method("POST");
        opts.set_mode(web_sys::RequestMode::Cors);
        
        // Create JSON body
        let body = serde_json::json!({
            "servers": servers
        });
        
        if let Ok(body_str) = serde_json::to_string(&body) {
            opts.set_body(&wasm_bindgen::JsValue::from_str(&body_str));
            opts.set_headers(&{
                let headers = web_sys::Headers::new().unwrap();
                headers.set("Content-Type", "application/json").unwrap();
                headers.into()
            });
            
            if let Ok(request) = web_sys::Request::new_with_str_and_init(&url, &opts) {
                let promise = window.fetch_with_request(&request);
                self.operation_promise = Some(wasm_bindgen_futures::JsFuture::from(promise));
                self.pending_operation = format!("bulk-restore:{}", servers.len());
                self.loading = true;
            }
        }
    }
    
    fn render_statistics(&self, ui: &mut egui::Ui, is_mobile: bool) {
        let stats = self.admin_state.get_statistics();
        
        ui.group(|ui| {
            ui.set_min_width(ui.available_width());
            ui.vertical(|ui| {
                let title_size = if is_mobile { 16.0 } else { 20.0 };
                ui.label(egui::RichText::new("📊 Statistics").size(title_size).strong());
                ui.add_space(if is_mobile { 10.0 } else { 15.0 });
                
                // Use 2x2 grid on mobile for better readability
                if is_mobile {
                    ui.columns(2, |cols| {
                        // Total keys
                        cols[0].vertical_centered_justified(|ui| {
                            ui.label(egui::RichText::new("📊").size(24.0));
                            ui.label(
                                egui::RichText::new(stats.total_keys.to_string())
                                    .size(28.0)
                                    .strong(),
                            );
                            ui.label(
                                egui::RichText::new("Total Keys")
                                    .size(12.0)
                                    .color(egui::Color32::GRAY),
                            );
                        });
                        
                        // Active keys - using original admin colors
                        cols[1].vertical_centered_justified(|ui| {
                            ui.label(egui::RichText::new("✅").size(24.0));
                            ui.label(
                                egui::RichText::new(stats.active_keys.to_string())
                                    .size(28.0)
                                    .strong()
                                    .color(egui::Color32::from_rgb(46, 204, 113)),
                            );
                            ui.label(
                                egui::RichText::new("Active")
                                    .size(12.0)
                                    .color(egui::Color32::GRAY),
                            );
                        });
                    });
                    
                    ui.add_space(10.0);
                    
                    ui.columns(2, |cols| {
                        // Deprecated keys - using original admin colors
                        cols[0].vertical_centered_justified(|ui| {
                            ui.label(egui::RichText::new("❌").size(24.0));
                            ui.label(
                                egui::RichText::new(stats.deprecated_keys.to_string())
                                    .size(28.0)
                                    .strong()
                                    .color(egui::Color32::from_rgb(231, 76, 60)),
                            );
                            ui.label(
                                egui::RichText::new("Deprecated")
                                    .size(12.0)
                                    .color(egui::Color32::GRAY),
                            );
                        });
                        
                        // Servers - using original admin colors
                        cols[1].vertical_centered_justified(|ui| {
                            ui.label(egui::RichText::new("💻").size(24.0));
                            ui.label(
                                egui::RichText::new(stats.unique_servers.to_string())
                                    .size(28.0)
                                    .strong()
                                    .color(egui::Color32::from_rgb(52, 152, 219)),
                            );
                            ui.label(
                                egui::RichText::new("Servers")
                                    .size(12.0)
                                    .color(egui::Color32::GRAY),
                            );
                        });
                    });
                } else {
                    ui.horizontal(|ui| {
                        ui.columns(4, |cols| {
                            // Total keys
                            cols[0].vertical_centered_justified(|ui| {
                                ui.label(egui::RichText::new("📊").size(32.0));
                                ui.add_space(5.0);
                                ui.label(
                                    egui::RichText::new(stats.total_keys.to_string())
                                        .size(36.0)
                                        .strong(),
                                );
                                ui.label(
                                    egui::RichText::new("Total Keys")
                                        .size(14.0)
                                        .color(egui::Color32::GRAY),
                                );
                            });
                            
                            // Active keys - using original admin colors
                            cols[1].vertical_centered_justified(|ui| {
                                ui.label(egui::RichText::new("✅").size(32.0));
                                ui.add_space(5.0);
                                ui.label(
                                    egui::RichText::new(stats.active_keys.to_string())
                                        .size(36.0)
                                        .strong()
                                        .color(egui::Color32::from_rgb(46, 204, 113)),
                                );
                                ui.label(
                                    egui::RichText::new("Active")
                                        .size(14.0)
                                        .color(egui::Color32::GRAY),
                                );
                            });
                            
                            // Deprecated keys - using original admin colors
                            cols[2].vertical_centered_justified(|ui| {
                                ui.label(egui::RichText::new("❌").size(32.0));
                                ui.add_space(5.0);
                                ui.label(
                                    egui::RichText::new(stats.deprecated_keys.to_string())
                                        .size(36.0)
                                        .strong()
                                        .color(egui::Color32::from_rgb(231, 76, 60)),
                                );
                                ui.label(
                                    egui::RichText::new("Deprecated")
                                        .size(14.0)
                                        .color(egui::Color32::GRAY),
                                );
                            });
                            
                            // Servers - using original admin colors
                            cols[3].vertical_centered_justified(|ui| {
                                ui.label(egui::RichText::new("💻").size(32.0));
                                ui.add_space(5.0);
                                ui.label(
                                    egui::RichText::new(stats.unique_servers.to_string())
                                        .size(36.0)
                                        .strong()
                                        .color(egui::Color32::from_rgb(52, 152, 219)),
                                );
                                ui.label(
                                    egui::RichText::new("Servers")
                                        .size(14.0)
                                        .color(egui::Color32::GRAY),
                                );
                            });
                        });
                    });
                }
            });
        });
    }
    
    fn render_search_controls(&mut self, ui: &mut egui::Ui, is_mobile: bool) {
        ui.group(|ui| {
            ui.set_min_width(ui.available_width());
            ui.vertical(|ui| {
                let title_size = if is_mobile { 16.0 } else { 20.0 };
                ui.label(egui::RichText::new("🔍 Search & Filter").size(title_size).strong());
                ui.add_space(if is_mobile { 8.0 } else { 12.0 });
                
                // Search field
                if is_mobile {
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new("🔍 Search").size(14.0));
                        let search_response = ui.add_sized(
                            [ui.available_width(), 36.0], // Larger touch target
                            egui::TextEdit::singleline(&mut self.admin_state.search_term)
                                .hint_text("Search servers or keys...")
                                .font(egui::FontId::proportional(16.0)),
                        );
                        
                        ui.add_space(5.0);
                        
                        if self.admin_state.search_term.is_empty() {
                            ui.label(
                                egui::RichText::new("Type to search")
                                    .size(12.0)
                                    .color(egui::Color32::GRAY),
                            );
                        } else {
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new(format!("{} results", self.admin_state.filtered_keys.len()))
                                        .size(12.0),
                                );
                                if ui.add_sized([60.0, 32.0], egui::Button::new(egui::RichText::new("❌ Clear").size(12.0))).clicked() {
                                    self.admin_state.search_term.clear();
                                    self.admin_state.filter_keys();
                                }
                            });
                        }
                        
                        if search_response.changed() {
                            self.admin_state.filter_keys();
                        }
                    });
                } else {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("🔍").size(18.0));
                        let search_response = ui.add_sized(
                            [ui.available_width() * 0.6, 28.0],
                            egui::TextEdit::singleline(&mut self.admin_state.search_term)
                                .hint_text("Search servers or keys...")
                                .font(egui::FontId::proportional(16.0)),
                        );
                        
                        if self.admin_state.search_term.is_empty() {
                            ui.label(
                                egui::RichText::new("Type to search")
                                    .size(14.0)
                                    .color(egui::Color32::GRAY),
                            );
                        } else {
                            ui.label(
                                egui::RichText::new(format!("{} results", self.admin_state.filtered_keys.len()))
                                    .size(14.0),
                            );
                            if ui.add_sized([35.0, 28.0], egui::Button::new(egui::RichText::new("❌").size(14.0))).on_hover_text("Clear search").clicked() {
                                self.admin_state.search_term.clear();
                                self.admin_state.filter_keys();
                            }
                        }
                        
                        if search_response.changed() {
                            self.admin_state.filter_keys();
                        }
                    });
                }
                
                ui.add_space(if is_mobile { 8.0 } else { 10.0 });
                
                // Filter buttons - using original admin colors
                let show_all = !self.admin_state.show_deprecated_only && !self.admin_state.show_active_only;
                let show_active = self.admin_state.show_active_only;
                let show_deprecated = self.admin_state.show_deprecated_only;
                
                if is_mobile {
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new("Filter:").size(14.0));
                        ui.add_space(5.0);
                        
                        if ui.add_sized([ui.available_width(), 40.0], egui::Button::new(egui::RichText::new("📋 All Keys").size(14.0)
                            .color(if show_all { egui::Color32::WHITE } else { egui::Color32::BLACK }))
                            .fill(if show_all { egui::Color32::from_rgb(52, 152, 219) } else { egui::Color32::GRAY })).clicked() {
                            self.admin_state.show_deprecated_only = false;
                            self.admin_state.show_active_only = false;
                            self.admin_state.filter_keys();
                        }
                        
                        ui.add_space(5.0);
                        
                        if ui.add_sized([ui.available_width(), 40.0], egui::Button::new(egui::RichText::new("✅ Active Only").size(14.0)
                            .color(if show_active { egui::Color32::WHITE } else { egui::Color32::BLACK }))
                            .fill(if show_active { egui::Color32::from_rgb(46, 204, 113) } else { egui::Color32::GRAY })).clicked() {
                            self.admin_state.show_deprecated_only = false;
                            self.admin_state.show_active_only = true;
                            self.admin_state.filter_keys();
                        }
                        
                        ui.add_space(5.0);
                        
                        if ui.add_sized([ui.available_width(), 40.0], egui::Button::new(egui::RichText::new("❗ Deprecated Only").size(14.0)
                            .color(if show_deprecated { egui::Color32::WHITE } else { egui::Color32::BLACK }))
                            .fill(if show_deprecated { egui::Color32::from_rgb(231, 76, 60) } else { egui::Color32::GRAY })).clicked() {
                            self.admin_state.show_deprecated_only = true;
                            self.admin_state.show_active_only = false;
                            self.admin_state.filter_keys();
                        }
                    });
                } else {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Filter:").size(16.0));
                        ui.add_space(10.0);
                        
                        if ui.add_sized([80.0, 32.0], egui::Button::new(egui::RichText::new("📋 All").size(14.0)
                            .color(if show_all { egui::Color32::WHITE } else { egui::Color32::BLACK }))
                            .fill(if show_all { egui::Color32::from_rgb(52, 152, 219) } else { egui::Color32::GRAY })).clicked() {
                            self.admin_state.show_deprecated_only = false;
                            self.admin_state.show_active_only = false;
                            self.admin_state.filter_keys();
                        }
                        if ui.add_sized([100.0, 32.0], egui::Button::new(egui::RichText::new("✅ Active").size(14.0)
                            .color(if show_active { egui::Color32::WHITE } else { egui::Color32::BLACK }))
                            .fill(if show_active { egui::Color32::from_rgb(46, 204, 113) } else { egui::Color32::GRAY })).clicked() {
                            self.admin_state.show_deprecated_only = false;
                            self.admin_state.show_active_only = true;
                            self.admin_state.filter_keys();
                        }
                        if ui.add_sized([120.0, 32.0], egui::Button::new(egui::RichText::new("❗ Deprecated").size(14.0)
                            .color(if show_deprecated { egui::Color32::WHITE } else { egui::Color32::BLACK }))
                            .fill(if show_deprecated { egui::Color32::from_rgb(231, 76, 60) } else { egui::Color32::GRAY })).clicked() {
                            self.admin_state.show_deprecated_only = true;
                            self.admin_state.show_active_only = false;
                            self.admin_state.filter_keys();
                        }
                    });
                }
            });
        });
    }
    
    fn render_bulk_actions(&mut self, ui: &mut egui::Ui, is_mobile: bool, button_height: f32) -> BulkAction {
        let selected_count = self.admin_state.selected_servers.values().filter(|&&v| v).count();
        
        if selected_count == 0 {
            return BulkAction::None;
        }
        
        let mut action = BulkAction::None;
        
        ui.group(|ui| {
            ui.set_min_width(ui.available_width());
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("📋").size(14.0));
                    ui.label(
                        egui::RichText::new(format!("Selected {} servers", selected_count))
                            .size(14.0)
                            .strong()
                            .color(egui::Color32::LIGHT_BLUE),
                    );
                });
                
                ui.add_space(5.0);
                
                // Use original admin colors for buttons
                if is_mobile {
                    ui.vertical(|ui| {
                        if ui.add_sized([ui.available_width(), button_height], egui::Button::new(egui::RichText::new("❗ Deprecate Selected").size(14.0)
                            .color(egui::Color32::BLACK))
                            .fill(egui::Color32::from_rgb(255, 200, 0))).clicked() {
                            action = BulkAction::DeprecateSelected;
                        }
                        
                        ui.add_space(5.0);
                        
                        if ui.add_sized([ui.available_width(), button_height], egui::Button::new(egui::RichText::new("✅ Restore Selected").size(14.0)
                            .color(egui::Color32::WHITE))
                            .fill(egui::Color32::from_rgb(101, 199, 40))).clicked() {
                            action = BulkAction::RestoreSelected;
                        }
                        
                        ui.add_space(5.0);
                        
                        if ui.add_sized([ui.available_width(), button_height], egui::Button::new(egui::RichText::new("❌ Clear Selection").size(14.0)
                            .color(egui::Color32::WHITE))
                            .fill(egui::Color32::from_rgb(170, 170, 170))).clicked() {
                            action = BulkAction::ClearSelection;
                        }
                    });
                } else {
                    ui.horizontal(|ui| {
                        if ui.add_sized([160.0, button_height], egui::Button::new(egui::RichText::new("❗ Deprecate Selected").size(14.0)
                            .color(egui::Color32::BLACK))
                            .fill(egui::Color32::from_rgb(255, 200, 0))).clicked() {
                            action = BulkAction::DeprecateSelected;
                        }
                        
                        ui.add_space(10.0);
                        
                        if ui.add_sized([140.0, button_height], egui::Button::new(egui::RichText::new("✅ Restore Selected").size(14.0)
                            .color(egui::Color32::WHITE))
                            .fill(egui::Color32::from_rgb(101, 199, 40))).clicked() {
                            action = BulkAction::RestoreSelected;
                        }
                        
                        ui.add_space(10.0);
                        
                        if ui.add_sized([120.0, button_height], egui::Button::new(egui::RichText::new("❌ Clear Selection").size(14.0)
                            .color(egui::Color32::WHITE))
                            .fill(egui::Color32::from_rgb(170, 170, 170))).clicked() {
                            action = BulkAction::ClearSelection;
                        }
                    });
                }
            });
        });
        
        action
    }
    
    fn render_keys_table(&mut self, ui: &mut egui::Ui, is_mobile: bool, button_height: f32) -> KeyAction {
        let mut action = KeyAction::None;
        
        // Group keys by server
        let mut servers: BTreeMap<String, Vec<SshKey>> = BTreeMap::new();
        for key in &self.admin_state.filtered_keys {
            servers
                .entry(key.server.clone())
                .or_insert_with(Vec::new)
                .push(key.clone());
        }
        
        // Render each server group
        for (server_name, server_keys) in servers {
            let is_expanded = self.admin_state
                .expanded_servers
                .get(&server_name)
                .copied()
                .unwrap_or(false);
            let active_count = server_keys.iter().filter(|k| !k.deprecated).count();
            let deprecated_count = server_keys.len() - active_count;
            
            // Server header
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    // Server selection checkbox
                    let mut selected = self.admin_state
                        .selected_servers
                        .get(&server_name)
                        .copied()
                        .unwrap_or(false);
                    if ui.checkbox(&mut selected, "").changed() {
                        self.admin_state
                            .selected_servers
                            .insert(server_name.clone(), selected);
                    }
                    
                    // Expand/collapse button
                    let expand_icon = if is_expanded { "-" } else { "+" };
                    if ui.small_button(expand_icon).clicked() {
                        self.admin_state
                            .expanded_servers
                            .insert(server_name.clone(), !is_expanded);
                    }
                    
                    // Server info
                    ui.label(egui::RichText::new("💻").size(16.0));
                    ui.label(
                        egui::RichText::new(&server_name)
                            .size(15.0)
                            .strong()
                            .color(egui::Color32::WHITE),
                    );
                    
                    ui.label(format!("{} keys", server_keys.len()));
                    
                    if deprecated_count > 0 {
                        ui.label(
                            egui::RichText::new(format!("{} depr", deprecated_count))
                                .color(egui::Color32::LIGHT_RED),
                        );
                    }
                    
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let server_button_size = if is_mobile { egui::vec2(80.0, 32.0) } else { egui::vec2(70.0, 24.0) };
                        
                        if deprecated_count > 0 {
                            if ui.add_sized(server_button_size, egui::Button::new(
                                egui::RichText::new("✅ Restore").color(egui::Color32::WHITE)
                            ).fill(egui::Color32::from_rgb(101, 199, 40))
                            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(94, 105, 25))))
                            .clicked() {
                                action = KeyAction::RestoreServer(server_name.clone());
                            }
                        }
                        
                        if active_count > 0 {
                            if ui.add_sized(server_button_size, egui::Button::new(
                                egui::RichText::new("❗ Deprecate").color(egui::Color32::BLACK)
                            ).fill(egui::Color32::from_rgb(255, 200, 0))
                            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(102, 94, 72))))
                            .clicked() {
                                action = KeyAction::DeprecateServer(server_name.clone());
                            }
                        }
                    });
                });
            });
            
            // Expanded key details
            if is_expanded {
                ui.indent("server_keys", |ui| {
                    for key in &server_keys {
                        if let Some(key_action) = self.render_key_item(ui, key, &server_name, is_mobile, button_height) {
                            action = key_action;
                        }
                    }
                });
            }
            
            ui.add_space(5.0);
        }
        
        action
    }
    
    fn render_key_item(&mut self, ui: &mut egui::Ui, key: &SshKey, server_name: &str, is_mobile: bool, _button_height: f32) -> Option<KeyAction> {
        let mut action = None;
        
        ui.group(|ui| {
            ui.horizontal(|ui| {
                // Key type badge
                let key_type = get_key_type(&key.public_key);
                ui.label(
                    egui::RichText::new(&key_type)
                        .size(10.0)
                        .color(egui::Color32::LIGHT_BLUE),
                );
                
                ui.add_space(5.0);
                
                // Status badge
                if key.deprecated {
                    ui.label(
                        egui::RichText::new("❗ DEPR")
                            .size(10.0)
                            .color(egui::Color32::from_rgb(231, 76, 60))
                            .strong(),
                    );
                } else {
                    ui.label(
                        egui::RichText::new("✅")
                            .size(10.0)
                            .color(egui::Color32::from_rgb(46, 204, 113))
                            .strong(),
                    );
                }
                
                ui.add_space(5.0);
                
                // Key preview
                ui.label(
                    egui::RichText::new(get_key_preview(&key.public_key))
                        .font(egui::FontId::monospace(10.0))
                        .color(egui::Color32::LIGHT_GRAY),
                );
                
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Key action buttons with original admin colors
                    let button_size = if is_mobile { egui::vec2(50.0, 32.0) } else { egui::vec2(40.0, 24.0) };
                    
                    if key.deprecated {
                        if ui.add_sized(button_size, egui::Button::new(
                            egui::RichText::new("R").color(egui::Color32::WHITE)
                        ).fill(egui::Color32::from_rgb(101, 199, 40))
                        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(94, 105, 25))))
                        .on_hover_text("Restore key").clicked() {
                            action = Some(KeyAction::RestoreKey(server_name.to_string()));
                        }
                        
                        if ui.add_sized(button_size, egui::Button::new(
                            egui::RichText::new("Del").color(egui::Color32::WHITE)
                        ).fill(egui::Color32::from_rgb(246, 36, 71))
                        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(129, 18, 17))))
                        .on_hover_text("Delete key").clicked() {
                            action = Some(KeyAction::DeleteKey(server_name.to_string()));
                        }
                    } else {
                        if ui.add_sized(button_size, egui::Button::new(
                            egui::RichText::new("❗").color(egui::Color32::BLACK)
                        ).fill(egui::Color32::from_rgb(255, 200, 0))
                        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(102, 94, 72))))
                        .on_hover_text("Deprecate key").clicked() {
                            action = Some(KeyAction::DeprecateKey(server_name.to_string()));
                        }
                    }
                    
                    if ui.add_sized(button_size, egui::Button::new(
                        egui::RichText::new("Copy").color(egui::Color32::WHITE)
                    ).fill(egui::Color32::from_rgb(0, 111, 230))
                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(35, 84, 97))))
                    .on_hover_text("Copy to clipboard").clicked() {
                        ui.output_mut(|o| o.copied_text = key.public_key.clone());
                    }
                });
            });
        });
        
        action
    }
    
    fn render_empty_state(&self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(60.0);
            if self.admin_state.keys.is_empty() {
                ui.label(
                    egui::RichText::new("🔑")
                        .size(48.0)
                        .color(egui::Color32::GRAY),
                );
                ui.label(
                    egui::RichText::new("No SSH keys available")
                        .size(18.0)
                        .color(egui::Color32::GRAY),
                );
                ui.label(
                    egui::RichText::new("Keys will appear here once loaded from the server")
                        .size(14.0)
                        .color(egui::Color32::DARK_GRAY),
                );
            } else if !self.admin_state.search_term.is_empty() {
                ui.label(
                    egui::RichText::new("🔍")
                        .size(48.0)
                        .color(egui::Color32::GRAY),
                );
                ui.label(
                    egui::RichText::new("No results found")
                        .size(18.0)
                        .color(egui::Color32::GRAY),
                );
                ui.label(
                    egui::RichText::new(format!(
                        "Try adjusting your search: '{}'",
                        self.admin_state.search_term
                    ))
                    .size(14.0)
                    .color(egui::Color32::DARK_GRAY),
                );
            } else {
                ui.label(
                    egui::RichText::new("❌")
                        .size(48.0)
                        .color(egui::Color32::GRAY),
                );
                ui.label(
                    egui::RichText::new("No keys match current filters")
                        .size(18.0)
                        .color(egui::Color32::GRAY),
                );
                ui.label(
                    egui::RichText::new("Try adjusting your search or filter settings")
                        .size(14.0)
                        .color(egui::Color32::DARK_GRAY),
                );
            }
        });
    }
    
    fn handle_bulk_action(&mut self, action: BulkAction) {
        match action {
            BulkAction::DeprecateSelected => {
                let selected = self.admin_state.get_selected_servers();
                if !selected.is_empty() {
                    self.bulk_deprecate_servers(selected);
                }
            }
            BulkAction::RestoreSelected => {
                let selected = self.admin_state.get_selected_servers();
                if !selected.is_empty() {
                    self.bulk_restore_servers(selected);
                }
            }
            BulkAction::ClearSelection => {
                self.admin_state.clear_selection();
                self.status_message = "Selection cleared".to_string();
            }
            BulkAction::None => {}
        }
    }
    
    fn handle_key_action(&mut self, action: KeyAction) {
        match action {
            KeyAction::DeprecateKey(server) => {
                self.deprecate_key(&server);
            }
            KeyAction::RestoreKey(server) => {
                self.restore_key(&server);
            }
            KeyAction::DeleteKey(server) => {
                self.delete_key(&server);
            }
            KeyAction::DeprecateServer(server) => {
                self.deprecate_key(&server);
            }
            KeyAction::RestoreServer(server) => {
                self.restore_key(&server);
            }
            KeyAction::None => {}
        }
    }
}

impl PartialEq for KeyAction {
    fn eq(&self, other: &Self) -> bool {
        matches!((self, other), (KeyAction::None, KeyAction::None))
    }
}

impl PartialEq for BulkAction {
    fn eq(&self, other: &Self) -> bool {
        matches!((self, other), (BulkAction::None, BulkAction::None))
    }
}

/// WASM entry point
#[wasm_bindgen]
pub fn start_web_admin(canvas_id: &str) -> Result<(), JsValue> {
    console_error_panic_hook::set_once();
    tracing_wasm::set_as_global_default();
    
    let web_options = eframe::WebOptions::default();
    let canvas_id = canvas_id.to_string();
    
    wasm_bindgen_futures::spawn_local(async move {
        let app = WebAdminApp::default();
        
        // Get the canvas element
        let document = web_sys::window()
            .unwrap()
            .document()
            .unwrap();
        
        let canvas = document
            .get_element_by_id(&canvas_id)
            .unwrap()
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .unwrap();
        
        let result = eframe::WebRunner::new()
            .start(
                canvas,
                web_options,
                Box::new(|_cc| Ok(Box::new(app))),
            )
            .await;
            
        match result {
            Ok(_) => web_sys::console::log_1(&"KHM Web Admin started successfully".into()),
            Err(e) => web_sys::console::error_1(&format!("Failed to start KHM Web Admin: {:?}", e).into()),
        }
    });
    
    Ok(())
}

#[wasm_bindgen(start)]
pub fn wasm_main() {
    console_error_panic_hook::set_once();
}