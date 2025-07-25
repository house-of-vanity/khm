use log::{error, info};

#[cfg(feature = "gui")]
use notify::RecursiveMode;
#[cfg(feature = "gui")]
use notify_debouncer_mini::{new_debouncer, DebounceEventResult};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tray_icon::{menu::MenuEvent, TrayIcon};
use winit::{
    application::ApplicationHandler,
    event_loop::{EventLoop, EventLoopProxy},
};

#[cfg(target_os = "macos")]
use winit::platform::macos::EventLoopBuilderExtMacOS;

#[cfg(target_os = "linux")]
use gtk::glib;

// Channel for Linux tray communication

#[cfg(target_os = "linux")]
enum LinuxTrayCommand {
    CreateTray {
        settings: KhmSettings,
        sync_status: SyncStatus,
    },
    UpdateMenu {
        settings: KhmSettings,
    },
    SetTooltip {
        tooltip: String,
    },
    #[allow(dead_code)]
    Quit,
}

#[cfg(target_os = "linux")]
enum LinuxTrayResponse {
    TrayCreated {
        menu_ids: TrayMenuIds,
    },
    #[allow(dead_code)]
    MenuUpdated {
        menu_ids: TrayMenuIds,
    },
    Error(String),
}

use super::{
    create_tooltip, create_tray_icon, start_auto_sync_task, update_sync_status, update_tray_menu,
    SyncStatus, TrayMenuIds,
};
use crate::gui::common::{get_config_path, load_settings, perform_sync, KhmSettings};

pub struct TrayApplication {
    #[cfg(not(target_os = "linux"))]
    tray_icon: Option<TrayIcon>,
    #[cfg(target_os = "linux")]
    linux_tray_tx: Option<std::sync::mpsc::Sender<LinuxTrayCommand>>,
    #[cfg(target_os = "linux")]
    linux_tray_handle: Option<std::thread::JoinHandle<()>>,
    menu_ids: Option<TrayMenuIds>,
    settings: Arc<Mutex<KhmSettings>>,
    sync_status: Arc<Mutex<SyncStatus>>,
    #[cfg(feature = "gui")]
    _debouncer: Option<notify_debouncer_mini::Debouncer<notify::RecommendedWatcher>>,
    proxy: EventLoopProxy<crate::gui::UserEvent>,
    auto_sync_handle: Option<std::thread::JoinHandle<()>>,
}

impl TrayApplication {
    pub fn new(proxy: EventLoopProxy<crate::gui::UserEvent>) -> Self {
        Self {
            #[cfg(not(target_os = "linux"))]
            tray_icon: None,
            #[cfg(target_os = "linux")]
            linux_tray_tx: None,
            #[cfg(target_os = "linux")]
            linux_tray_handle: None,
            menu_ids: None,
            settings: Arc::new(Mutex::new(load_settings())),
            sync_status: Arc::new(Mutex::new(SyncStatus::default())),
            #[cfg(feature = "gui")]
            _debouncer: None,
            proxy,
            auto_sync_handle: None,
        }
    }

    #[cfg(feature = "gui")]
    fn setup_file_watcher(&mut self) {
        let config_path = get_config_path();
        let (tx, rx) = std::sync::mpsc::channel::<DebounceEventResult>();
        let proxy = self.proxy.clone();

        std::thread::spawn(move || {
            while let Ok(result) = rx.recv() {
                if let Ok(events) = result {
                    if events
                        .iter()
                        .any(|e| e.path.to_string_lossy().contains("khm_config.json"))
                    {
                        let _ = proxy.send_event(crate::gui::UserEvent::ConfigFileChanged);
                    }
                }
            }
        });

        if let Ok(mut debouncer) = new_debouncer(Duration::from_millis(500), tx) {
            if let Some(config_dir) = config_path.parent() {
                if debouncer
                    .watcher()
                    .watch(config_dir, RecursiveMode::NonRecursive)
                    .is_ok()
                {
                    info!("File watcher started");
                    self._debouncer = Some(debouncer);
                } else {
                    error!("Failed to start file watcher");
                }
            }
        }
    }

    fn handle_config_change(&mut self) {
        info!("Config file changed");
        let new_settings = load_settings();
        let old_interval = self.settings.lock().unwrap().auto_sync_interval_minutes;
        let new_interval = new_settings.auto_sync_interval_minutes;

        *self.settings.lock().unwrap() = new_settings;

        // Update menu
        #[cfg(not(target_os = "linux"))]
        if let Some(tray_icon) = &self.tray_icon {
            let settings = self.settings.lock().unwrap();
            let new_menu_ids = update_tray_menu(tray_icon, &settings);
            self.menu_ids = Some(new_menu_ids);
        }
        
        #[cfg(target_os = "linux")]
        if let Some(ref tx) = self.linux_tray_tx {
            let settings = self.settings.lock().unwrap().clone();
            let _ = tx.send(LinuxTrayCommand::UpdateMenu { settings });
        }

        // Update tooltip
        self.update_tooltip();

        // Restart auto sync if interval changed
        if old_interval != new_interval {
            info!(
                "Auto sync interval changed from {} to {} minutes, restarting auto sync",
                old_interval, new_interval
            );
            self.start_auto_sync();
        }
    }

    fn start_auto_sync(&mut self) {
        if let Some(handle) = self.auto_sync_handle.take() {
            // Note: In a real implementation, you'd want to properly signal the thread to stop
            drop(handle);
        }

        self.auto_sync_handle = start_auto_sync_task(
            Arc::clone(&self.settings),
            Arc::clone(&self.sync_status),
            self.proxy.clone(),
        );
    }

    fn update_tooltip(&self) {
        let settings = self.settings.lock().unwrap();
        let sync_status = self.sync_status.lock().unwrap();
        let tooltip = create_tooltip(&settings, &sync_status);
        
        #[cfg(not(target_os = "linux"))]
        if let Some(tray_icon) = &self.tray_icon {
            let _ = tray_icon.set_tooltip(Some(&tooltip));
        }
        
        #[cfg(target_os = "linux")]
        if let Some(ref tx) = self.linux_tray_tx {
            let _ = tx.send(LinuxTrayCommand::SetTooltip { tooltip });
        }
    }

    fn handle_menu_event(
        &mut self,
        event: MenuEvent,
        event_loop: &winit::event_loop::ActiveEventLoop,
    ) {
        if let Some(menu_ids) = &self.menu_ids {
            if event.id == menu_ids.settings_id {
                info!("Settings menu clicked");
                self.launch_settings_window();
            } else if event.id == menu_ids.quit_id {
                info!("Quitting KHM application");
                event_loop.exit();
            } else if event.id == menu_ids.sync_id {
                info!("Starting manual sync operation");
                self.start_manual_sync();
            }
        }
    }

    fn launch_settings_window(&self) {
        if let Ok(exe_path) = std::env::current_exe() {
            std::thread::spawn(move || {
                if let Err(e) = std::process::Command::new(&exe_path)
                    .arg("--settings-ui")
                    .spawn()
                {
                    error!("Failed to launch settings window: {}", e);
                }
            });
        }
    }

    fn start_manual_sync(&self) {
        let settings = self.settings.lock().unwrap().clone();
        let sync_status_clone: Arc<Mutex<SyncStatus>> = Arc::clone(&self.sync_status);
        let proxy_clone = self.proxy.clone();

        // Check if settings are valid
        if settings.host.is_empty() || settings.flow.is_empty() {
            error!("Cannot sync: host or flow not configured");
            return;
        }

        info!(
            "Syncing with host: {}, flow: {}",
            settings.host, settings.flow
        );

        // Run sync in separate thread with its own tokio runtime
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                match perform_sync(&settings).await {
                    Ok(keys_count) => {
                        info!("Sync completed successfully with {} keys", keys_count);
                        let mut status = sync_status_clone.lock().unwrap();
                        status.last_sync_time = Some(std::time::Instant::now());
                        status.last_sync_keys = Some(keys_count);
                        let _ = proxy_clone.send_event(crate::gui::UserEvent::UpdateMenu);
                    }
                    Err(e) => {
                        error!("Sync failed: {}", e);
                    }
                }
            });
        });
    }

    fn handle_update_menu(&mut self) {
        let settings = self.settings.lock().unwrap();
        if !settings.host.is_empty() && !settings.flow.is_empty() && settings.in_place {
            let mut sync_status = self.sync_status.lock().unwrap();
            update_sync_status(&settings, &mut sync_status);
        }
        drop(settings);

        self.update_tooltip();
    }
}

impl ApplicationHandler<crate::gui::UserEvent> for TrayApplication {
    fn window_event(
        &mut self,
        _event_loop: &winit::event_loop::ActiveEventLoop,
        _window_id: winit::window::WindowId,
        _event: winit::event::WindowEvent,
    ) {
    }

    fn resumed(&mut self, _event_loop: &winit::event_loop::ActiveEventLoop) {
        #[cfg(not(target_os = "linux"))]
        if self.tray_icon.is_none() {
            info!("Creating tray icon");
            let settings = self.settings.lock().unwrap();
            let sync_status = self.sync_status.lock().unwrap();
            
            match std::panic::catch_unwind(|| create_tray_icon(&settings, &sync_status)) {
                Ok((tray_icon, menu_ids)) => {
                    drop(settings);
                    drop(sync_status);

                    self.tray_icon = Some(tray_icon);
                    self.menu_ids = Some(menu_ids);

                    self.setup_file_watcher();
                    self.start_auto_sync();
                    info!("KHM tray application ready");
                }
                Err(_) => {
                    drop(settings);
                    drop(sync_status);
                    error!("Failed to create tray icon. This usually means the required system libraries are not installed.");
                    error!("KHM will exit as system tray integration is required for desktop mode.");
                    std::process::exit(1);
                }
            }
        }
        
        #[cfg(target_os = "linux")]
        if self.linux_tray_tx.is_none() {
            info!("Creating tray icon on Linux");
            
            let (tx, rx) = std::sync::mpsc::channel();
            let (response_tx, response_rx) = std::sync::mpsc::channel();
            self.linux_tray_tx = Some(tx.clone());
            
            let proxy = self.proxy.clone();
            
            // Spawn GTK thread for tray
            let handle = std::thread::spawn(move || {
                if let Err(e) = gtk::init() {
                    error!("Failed to initialize GTK: {}", e);
                    let _ = response_tx.send(LinuxTrayResponse::Error(format!("GTK init failed: {}", e)));
                    return;
                }
                
                let mut tray_icon: Option<TrayIcon> = None;
                
                // Set up GTK event handlers
                let _tx_clone = tx.clone();
                glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
                    while let Ok(cmd) = rx.try_recv() {
                        match cmd {
                            LinuxTrayCommand::CreateTray { settings, sync_status } => {
                                match std::panic::catch_unwind(|| create_tray_icon(&settings, &sync_status)) {
                                    Ok((icon, menu_ids)) => {
                                        tray_icon = Some(icon);
                                        let _ = response_tx.send(LinuxTrayResponse::TrayCreated { menu_ids });
                                    }
                                    Err(_) => {
                                        let _ = response_tx.send(LinuxTrayResponse::Error("Failed to create tray".to_string()));
                                    }
                                }
                            }
                            LinuxTrayCommand::UpdateMenu { settings } => {
                                if let Some(ref icon) = tray_icon {
                                    let menu_ids = update_tray_menu(icon, &settings);
                                    let _ = response_tx.send(LinuxTrayResponse::MenuUpdated { menu_ids });
                                }
                            }
                            LinuxTrayCommand::SetTooltip { tooltip } => {
                                if let Some(ref icon) = tray_icon {
                                    let _ = icon.set_tooltip(Some(&tooltip));
                                }
                            }
                            LinuxTrayCommand::Quit => {
                                gtk::main_quit();
                                return glib::ControlFlow::Break;
                            }
                        }
                    }
                    
                    // Check for menu events
                    if let Ok(event) = MenuEvent::receiver().try_recv() {
                        let _ = proxy.send_event(crate::gui::UserEvent::MenuEvent(event));
                    }
                    
                    glib::ControlFlow::Continue
                });
                
                gtk::main();
            });
            
            self.linux_tray_handle = Some(handle);
            
            // Send command to create tray
            let settings = self.settings.lock().unwrap().clone();
            let sync_status = self.sync_status.lock().unwrap().clone();
            
            if let Some(ref tx) = self.linux_tray_tx {
                let _ = tx.send(LinuxTrayCommand::CreateTray { settings, sync_status });
                
                // Wait for response
                match response_rx.recv_timeout(std::time::Duration::from_secs(5)) {
                    Ok(LinuxTrayResponse::TrayCreated { menu_ids }) => {
                        self.menu_ids = Some(menu_ids);
                        self.setup_file_watcher();
                        self.start_auto_sync();
                        info!("KHM tray application ready");
                    }
                    Ok(LinuxTrayResponse::Error(e)) => {
                        error!("Failed to create tray icon: {}", e);
                        error!("This usually means the required system libraries are not installed.");
                        error!("On Ubuntu/Debian, try installing: sudo apt install libayatana-appindicator3-1");
                        error!("Alternative: sudo apt install libappindicator3-1");
                        std::process::exit(1);
                    }
                    _ => {
                        error!("Timeout waiting for tray creation");
                        std::process::exit(1);
                    }
                }
            }
        }
    }

    fn user_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        event: crate::gui::UserEvent,
    ) {
        match event {
            crate::gui::UserEvent::TrayIconEvent => {}
            crate::gui::UserEvent::UpdateMenu => {
                self.handle_update_menu();
            }
            crate::gui::UserEvent::MenuEvent(event) => {
                self.handle_menu_event(event, event_loop);
            }
            crate::gui::UserEvent::ConfigFileChanged => {
                self.handle_config_change();
            }
        }
    }
}

/// Run tray application
pub async fn run_tray_app() -> std::io::Result<()> {
    #[cfg(target_os = "macos")]
    let event_loop = {
        use winit::platform::macos::ActivationPolicy;
        EventLoop::<crate::gui::UserEvent>::with_user_event()
            .with_activation_policy(ActivationPolicy::Accessory)
            .build()
            .map_err(|e| {
                std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("Failed to create event loop: {}", e),
                )
            })?
    };

    #[cfg(not(target_os = "macos"))]
    let event_loop = EventLoop::<crate::gui::UserEvent>::with_user_event()
        .build()
        .map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to create event loop: {}", e),
            )
        })?;

    let proxy = event_loop.create_proxy();

    // Setup event handlers
    let proxy_clone = proxy.clone();
    tray_icon::TrayIconEvent::set_event_handler(Some(move |_event| {
        let _ = proxy_clone.send_event(crate::gui::UserEvent::TrayIconEvent);
    }));

    let proxy_clone = proxy.clone();
    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        let _ = proxy_clone.send_event(crate::gui::UserEvent::MenuEvent(event));
    }));

    let mut app = TrayApplication::new(proxy);

    event_loop.run_app(&mut app).map_err(|e| {
        std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("Event loop error: {:?}", e),
        )
    })?;

    Ok(())
}
