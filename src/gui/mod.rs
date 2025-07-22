use log::{debug, error, info};
use notify::RecursiveMode;
use notify_debouncer_mini::{new_debouncer, DebounceEventResult};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem, MenuId},
    TrayIcon, TrayIconBuilder,
};
use winit::{
    application::ApplicationHandler,
    event_loop::{EventLoop, EventLoopProxy},
};

#[cfg(target_os = "macos")]
use winit::platform::macos::EventLoopBuilderExtMacOS;

mod settings;
pub use settings::{KhmSettings, load_settings};

// Function to run settings window (for --settings-ui mode)
pub fn run_settings_window() {
    settings::run_settings_window();
}

// Function to perform sync operation using KHM client logic
async fn perform_sync(settings: &KhmSettings) -> Result<usize, std::io::Error> {
    use crate::Args;
    
    info!("Starting sync with settings: host={}, flow={}, known_hosts={}, in_place={}", 
          settings.host, settings.flow, settings.known_hosts, settings.in_place);
    
    // Convert KhmSettings to Args for client module
    let args = Args {
        server: false,
        gui: false,
        settings_ui: false,
        in_place: settings.in_place,
        flows: vec!["default".to_string()], // Not used in client mode
        ip: "127.0.0.1".to_string(), // Not used in client mode
        port: 8080, // Not used in client mode
        db_host: "127.0.0.1".to_string(), // Not used in client mode
        db_name: "khm".to_string(), // Not used in client mode
        db_user: None, // Not used in client mode
        db_password: None, // Not used in client mode
        host: Some(settings.host.clone()),
        flow: Some(settings.flow.clone()),
        known_hosts: settings::expand_path(&settings.known_hosts),
        basic_auth: settings.basic_auth.clone(),
    };
    
    info!("Expanded known_hosts path: {}", args.known_hosts);
    
    // Get keys count before and after sync
    let keys_before = crate::client::read_known_hosts(&args.known_hosts)
        .unwrap_or_else(|_| Vec::new())
        .len();
    
    crate::client::run_client(args.clone()).await?;
    
    let keys_after = if args.in_place {
        crate::client::read_known_hosts(&args.known_hosts)
            .unwrap_or_else(|_| Vec::new())
            .len()
    } else {
        keys_before
    };
    
    info!("Sync completed: {} keys before, {} keys after", keys_before, keys_after);
    Ok(keys_after)
}

#[derive(Debug)]
enum UserEvent {
    TrayIconEvent,
    MenuEvent(tray_icon::menu::MenuEvent),
    ConfigFileChanged,
    UpdateMenu,
}

#[derive(Debug, Clone)]
struct SyncStatus {
    last_sync_time: Option<std::time::Instant>,
    last_sync_keys: Option<usize>,
    next_sync_in_seconds: Option<u64>,
}

impl Default for SyncStatus {
    fn default() -> Self {
        Self {
            last_sync_time: None,
            last_sync_keys: None,
            next_sync_in_seconds: None,
        }
    }
}

fn create_tray_icon(settings: &KhmSettings, sync_status: &SyncStatus) -> (TrayIcon, MenuId, MenuId, MenuId) {
    // Create simple blue icon with "KHM" text representation
    let icon_data: Vec<u8> = (0..32*32).flat_map(|i| {
        let y = i / 32;
        let x = i % 32;
        if x < 2 || x >= 30 || y < 2 || y >= 30 {
            [255, 255, 255, 255] // White border
        } else {
            [64, 128, 255, 255] // Blue center
        }
    }).collect();
    
    let icon = tray_icon::Icon::from_rgba(icon_data, 32, 32).unwrap();
    let menu = Menu::new();
    
    // Show current configuration status (static)
    let host_text = if settings.host.is_empty() {
        "Host: Not configured"
    } else {
        &format!("Host: {}", settings.host)
    };
    menu.append(&MenuItem::new(host_text, false, None)).unwrap();
    
    let flow_text = if settings.flow.is_empty() {
        "Flow: Not configured"
    } else {
        &format!("Flow: {}", settings.flow)
    };
    menu.append(&MenuItem::new(flow_text, false, None)).unwrap();
    
    let is_auto_sync_enabled = !settings.host.is_empty() && !settings.flow.is_empty() && settings.in_place;
    let sync_text = format!("Auto sync: {} ({}min)", 
                           if is_auto_sync_enabled { "On" } else { "Off" },
                           settings.auto_sync_interval_minutes);
    menu.append(&MenuItem::new(&sync_text, false, None)).unwrap();
    
    menu.append(&tray_icon::menu::PredefinedMenuItem::separator()).unwrap();
    
    // Sync Now menu item
    let sync_item = MenuItem::new("Sync Now", !settings.host.is_empty() && !settings.flow.is_empty(), None);
    let sync_id = sync_item.id().clone();
    menu.append(&sync_item).unwrap();
    
    menu.append(&tray_icon::menu::PredefinedMenuItem::separator()).unwrap();
    
    // Settings menu item
    let settings_item = MenuItem::new("Settings", true, None);
    let settings_id = settings_item.id().clone();
    menu.append(&settings_item).unwrap();
    
    menu.append(&tray_icon::menu::PredefinedMenuItem::separator()).unwrap();
    
    // Quit menu item
    let quit_item = MenuItem::new("Quit", true, None);
    let quit_id = quit_item.id().clone();
    menu.append(&quit_item).unwrap();
    
    // Create initial tooltip
    let mut tooltip = format!("KHM - SSH Key Manager\nHost: {}\nFlow: {}", settings.host, settings.flow);
    if let Some(keys_count) = sync_status.last_sync_keys {
        tooltip.push_str(&format!("\nLast sync: {} keys", keys_count));
    } else {
        tooltip.push_str("\nLast sync: Never");
    }
    
    let tray_icon = TrayIconBuilder::new()
        .with_tooltip(&tooltip)
        .with_icon(icon)
        .with_menu(Box::new(menu))
        .build()
        .unwrap();
    
    (tray_icon, settings_id, quit_id, sync_id)
}

struct Application {
    tray_icon: Option<TrayIcon>,
    settings_id: Option<MenuId>,
    quit_id: Option<MenuId>,
    sync_id: Option<MenuId>,
    settings: Arc<Mutex<KhmSettings>>,
    sync_status: Arc<Mutex<SyncStatus>>,
    _debouncer: Option<notify_debouncer_mini::Debouncer<notify::FsEventWatcher>>,
    proxy: EventLoopProxy<UserEvent>,
    auto_sync_handle: Option<std::thread::JoinHandle<()>>,
}

impl Application {
    fn new(proxy: EventLoopProxy<UserEvent>) -> Self {
        Self {
            tray_icon: None,
            settings_id: None,
            quit_id: None,
            sync_id: None,
            settings: Arc::new(Mutex::new(load_settings())),
            sync_status: Arc::new(Mutex::new(SyncStatus::default())),
            _debouncer: None,
            proxy,
            auto_sync_handle: None,
        }
    }
    
    fn update_menu(&mut self) {
        if let Some(tray_icon) = &self.tray_icon {
            let settings = self.settings.lock().unwrap();
            let menu = Menu::new();
            
            // Show current configuration status (static)
            let host_text = if settings.host.is_empty() {
                "Host: Not configured"
            } else {
                &format!("Host: {}", settings.host)
            };
            menu.append(&MenuItem::new(host_text, false, None)).unwrap();
            
            let flow_text = if settings.flow.is_empty() {
                "Flow: Not configured"
            } else {
                &format!("Flow: {}", settings.flow)
            };
            menu.append(&MenuItem::new(flow_text, false, None)).unwrap();
            
            let is_auto_sync_enabled = !settings.host.is_empty() && !settings.flow.is_empty() && settings.in_place;
            let sync_text = format!("Auto sync: {} ({}min)", 
                                   if is_auto_sync_enabled { "On" } else { "Off" },
                                   settings.auto_sync_interval_minutes);
            menu.append(&MenuItem::new(&sync_text, false, None)).unwrap();
            
            menu.append(&tray_icon::menu::PredefinedMenuItem::separator()).unwrap();
            
            // Sync Now menu item
            let sync_item = MenuItem::new("Sync Now", !settings.host.is_empty() && !settings.flow.is_empty(), None);
            let new_sync_id = sync_item.id().clone();
            menu.append(&sync_item).unwrap();
            
            menu.append(&tray_icon::menu::PredefinedMenuItem::separator()).unwrap();
            
            // Settings menu item
            let settings_item = MenuItem::new("Settings", true, None);
            let new_settings_id = settings_item.id().clone();
            menu.append(&settings_item).unwrap();
            
            menu.append(&tray_icon::menu::PredefinedMenuItem::separator()).unwrap();
            
            // Quit menu item
            let quit_item = MenuItem::new("Quit", true, None);
            let new_quit_id = quit_item.id().clone();
            menu.append(&quit_item).unwrap();
            
            tray_icon.set_menu(Some(Box::new(menu)));
            self.settings_id = Some(new_settings_id);
            self.quit_id = Some(new_quit_id);
            self.sync_id = Some(new_sync_id);
        }
    }
    
    fn setup_file_watcher(&mut self) {
        let config_path = settings::get_config_path();
        let (tx, rx) = std::sync::mpsc::channel::<DebounceEventResult>();
        let proxy = self.proxy.clone();
        
        std::thread::spawn(move || {
            while let Ok(result) = rx.recv() {
                if let Ok(events) = result {
                    if events.iter().any(|e| e.path.to_string_lossy().contains("khm_config.json")) {
                        let _ = proxy.send_event(UserEvent::ConfigFileChanged);
                    }
                }
            }
        });
        
        if let Ok(mut debouncer) = new_debouncer(Duration::from_millis(500), tx) {
            if let Some(config_dir) = config_path.parent() {
                if debouncer.watcher().watch(config_dir, RecursiveMode::NonRecursive).is_ok() {
                    debug!("File watcher started");
                    self._debouncer = Some(debouncer);
                } else {
                    error!("Failed to start file watcher");
                }
            }
        }
    }
    
    fn start_auto_sync(&mut self) {
        let settings = self.settings.lock().unwrap().clone();
        
        // Only start auto sync if settings are valid and in_place is enabled
        if settings.host.is_empty() || settings.flow.is_empty() || !settings.in_place {
            info!("Auto sync disabled or settings invalid");
            return;
        }
        
        info!("Starting auto sync with interval {} minutes", settings.auto_sync_interval_minutes);
        
        let settings_clone = Arc::clone(&self.settings);
        let sync_status_clone = Arc::clone(&self.sync_status);
        let proxy_clone = self.proxy.clone();
        let interval_minutes = settings.auto_sync_interval_minutes;
        
        let handle = std::thread::spawn(move || {
            // Initial sync on startup
            info!("Performing initial sync on startup");
            let current_settings = settings_clone.lock().unwrap().clone();
            if !current_settings.host.is_empty() && !current_settings.flow.is_empty() {
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async {
                    match perform_sync(&current_settings).await {
                        Ok(keys_count) => {
                            info!("Initial sync completed successfully with {} keys", keys_count);
                            let mut status = sync_status_clone.lock().unwrap();
                            status.last_sync_time = Some(std::time::Instant::now());
                            status.last_sync_keys = Some(keys_count);
                            let _ = proxy_clone.send_event(UserEvent::UpdateMenu);
                        }
                        Err(e) => {
                            error!("Initial sync failed: {}", e);
                        }
                    }
                });
            }
            
            // Start menu update timer
            let proxy_timer = proxy_clone.clone();
            std::thread::spawn(move || {
                loop {
                    std::thread::sleep(std::time::Duration::from_secs(1));
                    let _ = proxy_timer.send_event(UserEvent::UpdateMenu);
                }
            });
            
            // Periodic sync
            loop {
                std::thread::sleep(std::time::Duration::from_secs(interval_minutes as u64 * 60));
                
                let current_settings = settings_clone.lock().unwrap().clone();
                if current_settings.host.is_empty() || current_settings.flow.is_empty() || !current_settings.in_place {
                    info!("Auto sync stopped due to invalid settings or disabled in_place");
                    break;
                }
                
                info!("Performing scheduled auto sync");
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async {
                    match perform_sync(&current_settings).await {
                        Ok(keys_count) => {
                            info!("Auto sync completed successfully with {} keys", keys_count);
                            let mut status = sync_status_clone.lock().unwrap();
                            status.last_sync_time = Some(std::time::Instant::now());
                            status.last_sync_keys = Some(keys_count);
                            let _ = proxy_clone.send_event(UserEvent::UpdateMenu);
                        }
                        Err(e) => {
                            error!("Auto sync failed: {}", e);
                        }
                    }
                });
            }
        });
        
        self.auto_sync_handle = Some(handle);
    }
}

impl ApplicationHandler<UserEvent> for Application {
    fn window_event(
        &mut self,
        _event_loop: &winit::event_loop::ActiveEventLoop,
        _window_id: winit::window::WindowId,
        _event: winit::event::WindowEvent,
    ) {}
    
    fn resumed(&mut self, _event_loop: &winit::event_loop::ActiveEventLoop) {
        if self.tray_icon.is_none() {
            info!("Creating tray icon");
            let settings = self.settings.lock().unwrap();
            let sync_status = self.sync_status.lock().unwrap();
            let (tray_icon, settings_id, quit_id, sync_id) = create_tray_icon(&settings, &sync_status);
            drop(settings);
            drop(sync_status);
            
            self.tray_icon = Some(tray_icon);
            self.settings_id = Some(settings_id);
            self.quit_id = Some(quit_id);
            self.sync_id = Some(sync_id);
            
            self.setup_file_watcher();
            self.start_auto_sync();
            info!("KHM tray application ready");
        }
    }
    
    fn user_event(&mut self, event_loop: &winit::event_loop::ActiveEventLoop, event: UserEvent) {
        match event {
            UserEvent::TrayIconEvent => {}
            UserEvent::UpdateMenu => {
                // Update tooltip with sync status instead of menu items
                let settings = self.settings.lock().unwrap();
                if !settings.host.is_empty() && !settings.flow.is_empty() && settings.in_place {
                    let mut sync_status = self.sync_status.lock().unwrap();
                    if let Some(last_sync) = sync_status.last_sync_time {
                        let elapsed = last_sync.elapsed().as_secs();
                        let interval_seconds = settings.auto_sync_interval_minutes as u64 * 60;
                        
                        if elapsed < interval_seconds {
                            sync_status.next_sync_in_seconds = Some(interval_seconds - elapsed);
                        } else {
                            sync_status.next_sync_in_seconds = Some(0);
                        }
                    } else {
                        sync_status.next_sync_in_seconds = None;
                    }
                    
                    // Update tooltip with current status
                    if let Some(tray_icon) = &self.tray_icon {
                        let mut tooltip = format!("KHM - SSH Key Manager\nHost: {}\nFlow: {}", settings.host, settings.flow);
                        
                        if let Some(keys_count) = sync_status.last_sync_keys {
                            tooltip.push_str(&format!("\nLast sync: {} keys", keys_count));
                        } else {
                            tooltip.push_str("\nLast sync: Never");
                        }
                        
                        if let Some(seconds) = sync_status.next_sync_in_seconds {
                            if seconds > 60 {
                                tooltip.push_str(&format!("\nNext sync: {}m {}s", seconds / 60, seconds % 60));
                            } else {
                                tooltip.push_str(&format!("\nNext sync: {}s", seconds));
                            }
                        }
                        
                        let _ = tray_icon.set_tooltip(Some(&tooltip));
                    }
                }
                drop(settings);
            }
            UserEvent::MenuEvent(event) => {
                if let (Some(settings_id), Some(quit_id), Some(sync_id)) = (&self.settings_id, &self.quit_id, &self.sync_id) {
                    if event.id == *settings_id {
                        info!("Settings menu clicked");
                        if let Ok(exe_path) = std::env::current_exe() {
                            std::thread::spawn(move || {
                                if let Err(e) = std::process::Command::new(&exe_path)
                                    .arg("--gui")
                                    .arg("--settings-ui")
                                    .spawn()
                                {
                                    error!("Failed to launch settings window: {}", e);
                                }
                            });
                        }
                    } else if event.id == *quit_id {
                        info!("Quitting KHM application");
                        event_loop.exit();
                    } else if event.id == *sync_id {
                        info!("Starting sync operation");
                        let settings = self.settings.lock().unwrap().clone();
                        let sync_status_clone = Arc::clone(&self.sync_status);
                        let proxy_clone = self.proxy.clone();
                        
                        // Check if settings are valid
                        if settings.host.is_empty() || settings.flow.is_empty() {
                            error!("Cannot sync: host or flow not configured");
                        } else {
                            info!("Syncing with host: {}, flow: {}", settings.host, settings.flow);
                            
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
                                            let _ = proxy_clone.send_event(UserEvent::UpdateMenu);
                                        }
                                        Err(e) => {
                                            error!("Sync failed: {}", e);
                                        }
                                    }
                                });
                            });
                        }
                    }
                }
            }
            UserEvent::ConfigFileChanged => {
                debug!("Config file changed");
                let new_settings = load_settings();
                let old_interval = self.settings.lock().unwrap().auto_sync_interval_minutes;
                let new_interval = new_settings.auto_sync_interval_minutes;
                
                *self.settings.lock().unwrap() = new_settings;
                self.update_menu();
                
                // Update tooltip with new settings
                if let Some(tray_icon) = &self.tray_icon {
                    let settings = self.settings.lock().unwrap();
                    let sync_status = self.sync_status.lock().unwrap();
                    let mut tooltip = format!("KHM - SSH Key Manager\nHost: {}\nFlow: {}", settings.host, settings.flow);
                    
                    if let Some(keys_count) = sync_status.last_sync_keys {
                        tooltip.push_str(&format!("\nLast sync: {} keys", keys_count));
                    } else {
                        tooltip.push_str("\nLast sync: Never");
                    }
                    
                    if let Some(seconds) = sync_status.next_sync_in_seconds {
                        if seconds > 60 {
                            tooltip.push_str(&format!("\nNext sync: {}m {}s", seconds / 60, seconds % 60));
                        } else {
                            tooltip.push_str(&format!("\nNext sync: {}s", seconds));
                        }
                    }
                    
                    let _ = tray_icon.set_tooltip(Some(&tooltip));
                }
                
                // Restart auto sync if interval changed or settings changed
                if old_interval != new_interval {
                    info!("Auto sync interval changed from {} to {} minutes, restarting auto sync", old_interval, new_interval);
                    // Note: The auto sync thread will automatically stop and restart based on settings
                    self.start_auto_sync();
                }
            }
        }
    }
}

async fn run_tray() -> std::io::Result<()> {
    #[cfg(target_os = "macos")]
    let event_loop = {
        use winit::platform::macos::ActivationPolicy;
        EventLoop::<UserEvent>::with_user_event()
            .with_activation_policy(ActivationPolicy::Accessory)
            .build()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to create event loop: {}", e)))?
    };
    
    #[cfg(not(target_os = "macos"))]
    let event_loop = EventLoop::<UserEvent>::with_user_event().build()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to create event loop: {}", e)))?;
    
    let proxy = event_loop.create_proxy();
    
    let proxy_clone = proxy.clone();
    tray_icon::TrayIconEvent::set_event_handler(Some(move |_event| {
        let _ = proxy_clone.send_event(UserEvent::TrayIconEvent);
    }));
    
    let proxy_clone = proxy.clone();
    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        let _ = proxy_clone.send_event(UserEvent::MenuEvent(event));
    }));
    
    let mut app = Application::new(proxy);
    
    event_loop.run_app(&mut app)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Event loop error: {:?}", e)))?;
    
    Ok(())
}

pub async fn run_gui() -> std::io::Result<()> {
    info!("Starting KHM tray application");
    run_tray().await
}
