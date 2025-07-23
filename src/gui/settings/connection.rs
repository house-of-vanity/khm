use crate::gui::api::{perform_manual_sync, test_connection};
use crate::gui::common::{save_settings, KhmSettings};
use eframe::egui;
use log::{error, info};
use std::sync::mpsc;

#[derive(Debug, Clone)]
pub enum ConnectionStatus {
    Unknown,
    Connected { keys_count: usize, flow: String },
    Error(String),
}

#[derive(Debug, Clone)]
pub enum SyncStatus {
    Unknown,
    Success { keys_count: usize },
    Error(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum SettingsTab {
    Connection,
    Admin,
}

pub struct ConnectionTab {
    pub connection_status: ConnectionStatus,
    pub is_testing_connection: bool,
    pub test_result_receiver: Option<mpsc::Receiver<Result<String, String>>>,
    pub is_syncing: bool,
    pub sync_result_receiver: Option<mpsc::Receiver<Result<String, String>>>,
    pub sync_status: SyncStatus,
    pub should_auto_test: bool,
}

impl Default for ConnectionTab {
    fn default() -> Self {
        Self {
            connection_status: ConnectionStatus::Unknown,
            is_testing_connection: false,
            test_result_receiver: None,
            is_syncing: false,
            sync_result_receiver: None,
            sync_status: SyncStatus::Unknown,
            should_auto_test: false,
        }
    }
}

impl ConnectionTab {
    /// Start connection test
    pub fn start_test(&mut self, settings: &KhmSettings, ctx: &egui::Context) {
        if self.is_testing_connection {
            return;
        }

        self.is_testing_connection = true;
        self.connection_status = ConnectionStatus::Unknown;

        let (tx, rx) = mpsc::channel();
        self.test_result_receiver = Some(rx);

        let host = settings.host.clone();
        let flow = settings.flow.clone();
        let basic_auth = settings.basic_auth.clone();
        let ctx_clone = ctx.clone();

        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let result = rt.block_on(async { test_connection(host, flow, basic_auth).await });

            let _ = tx.send(result);
            ctx_clone.request_repaint();
        });
    }

    /// Start manual sync
    pub fn start_sync(&mut self, settings: &KhmSettings, ctx: &egui::Context) {
        if self.is_syncing {
            return;
        }

        self.is_syncing = true;
        self.sync_status = SyncStatus::Unknown;

        let (tx, rx) = mpsc::channel();
        self.sync_result_receiver = Some(rx);

        let settings = settings.clone();
        let ctx_clone = ctx.clone();

        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let result = rt.block_on(async { perform_manual_sync(settings).await });

            let _ = tx.send(result);
            ctx_clone.request_repaint();
        });
    }

    /// Check for test/sync results and handle auto-test
    pub fn check_results(
        &mut self,
        ctx: &egui::Context,
        settings: &KhmSettings,
        operation_log: &mut Vec<String>,
    ) {
        // Handle auto-test on first frame if needed
        if self.should_auto_test && !self.is_testing_connection {
            self.should_auto_test = false;
            self.start_test(settings, ctx);
        }
        // Check for test connection result
        if let Some(receiver) = &self.test_result_receiver {
            if let Ok(result) = receiver.try_recv() {
                self.is_testing_connection = false;
                match result {
                    Ok(message) => {
                        // Parse keys count from message
                        let keys_count = if let Some(start) = message.find("Found ") {
                            if let Some(end) = message[start + 6..].find(" SSH keys") {
                                message[start + 6..start + 6 + end]
                                    .parse::<usize>()
                                    .unwrap_or(0)
                            } else {
                                0
                            }
                        } else {
                            0
                        };

                        self.connection_status = ConnectionStatus::Connected {
                            keys_count,
                            flow: settings.flow.clone(),
                        };
                        info!("Connection test successful: {}", message);

                        // Add to UI log
                        super::ui::add_log_entry(
                            operation_log,
                            format!("✅ Connection test successful: {}", message),
                        );
                    }
                    Err(error) => {
                        self.connection_status = ConnectionStatus::Error(error.clone());
                        error!("Connection test failed");

                        // Add to UI log
                        super::ui::add_log_entry(
                            operation_log,
                            format!("❌ Connection test failed: {}", error),
                        );
                    }
                }
                self.test_result_receiver = None;
                ctx.request_repaint();
            }
        }

        // Check for sync result
        if let Some(receiver) = &self.sync_result_receiver {
            if let Ok(result) = receiver.try_recv() {
                self.is_syncing = false;
                match result {
                    Ok(message) => {
                        // Parse keys count from message
                        let keys_count = parse_keys_count(&message);
                        self.sync_status = SyncStatus::Success { keys_count };
                        info!("Sync successful: {}", message);

                        // Add to UI log
                        super::ui::add_log_entry(
                            operation_log,
                            format!("✅ Sync completed: {}", message),
                        );
                    }
                    Err(error) => {
                        self.sync_status = SyncStatus::Error(error.clone());
                        error!("Sync failed");

                        // Add to UI log
                        super::ui::add_log_entry(
                            operation_log,
                            format!("❌ Sync failed: {}", error),
                        );
                    }
                }
                self.sync_result_receiver = None;
                ctx.request_repaint();
            }
        }
    }
}

/// Parse keys count from sync result message
fn parse_keys_count(message: &str) -> usize {
    if let Some(start) = message.find("updated with ") {
        let search_start = start + "updated with ".len();
        if let Some(end) = message[search_start..].find(" keys") {
            let number_str = &message[search_start..search_start + end];
            return number_str.parse::<usize>().unwrap_or(0);
        }
    } else if let Some(start) = message.find("Retrieved ") {
        let search_start = start + "Retrieved ".len();
        if let Some(end) = message[search_start..].find(" keys") {
            let number_str = &message[search_start..search_start + end];
            return number_str.parse::<usize>().unwrap_or(0);
        }
    } else if let Some(keys_pos) = message.find(" keys") {
        let before_keys = &message[..keys_pos];
        if let Some(space_pos) = before_keys.rfind(' ') {
            let number_str = &before_keys[space_pos + 1..];
            return number_str.parse::<usize>().unwrap_or(0);
        }
    }

    0
}

/// Save settings with validation
pub fn save_settings_validated(settings: &KhmSettings) -> Result<(), String> {
    if settings.host.is_empty() || settings.flow.is_empty() {
        return Err("Host URL and Flow Name are required".to_string());
    }

    save_settings(settings).map_err(|e| format!("Failed to save settings: {}", e))
}
