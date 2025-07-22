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
async fn perform_sync(settings: &KhmSettings) -> std::io::Result<()> {
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
    
    crate::client::run_client(args).await
}

#[derive(Debug)]
enum UserEvent {
    TrayIconEvent,
    MenuEvent(tray_icon::menu::MenuEvent),
    ConfigFileChanged,
}

fn create_tray_icon(settings: &KhmSettings) -> (TrayIcon, MenuId, MenuId, MenuId) {
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
    
    // Show current configuration status
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
    
    let sync_text = format!("Auto sync: {}", if settings.in_place { "On" } else { "Off" });
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
    
    let tray_icon = TrayIconBuilder::new()
        .with_tooltip("KHM - SSH Key Manager")
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
    _debouncer: Option<notify_debouncer_mini::Debouncer<notify::FsEventWatcher>>,
    proxy: EventLoopProxy<UserEvent>,
}

impl Application {
    fn new(proxy: EventLoopProxy<UserEvent>) -> Self {
        Self {
            tray_icon: None,
            settings_id: None,
            quit_id: None,
            sync_id: None,
            settings: Arc::new(Mutex::new(load_settings())),
            _debouncer: None,
            proxy,
        }
    }
    
    fn update_menu(&mut self) {
        if let Some(tray_icon) = &self.tray_icon {
            let settings = self.settings.lock().unwrap();
            let menu = Menu::new();
            
            // Show current configuration status
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
            
            let sync_text = format!("Auto sync: {}", if settings.in_place { "On" } else { "Off" });
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
            let (tray_icon, settings_id, quit_id, sync_id) = create_tray_icon(&settings);
            drop(settings);
            
            self.tray_icon = Some(tray_icon);
            self.settings_id = Some(settings_id);
            self.quit_id = Some(quit_id);
            self.sync_id = Some(sync_id);
            
            self.setup_file_watcher();
            info!("KHM tray application ready");
        }
    }
    
    fn user_event(&mut self, event_loop: &winit::event_loop::ActiveEventLoop, event: UserEvent) {
        match event {
            UserEvent::TrayIconEvent => {}
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
                        
                        // Check if settings are valid
                        if settings.host.is_empty() || settings.flow.is_empty() {
                            error!("Cannot sync: host or flow not configured");
                        } else {
                            info!("Syncing with host: {}, flow: {}", settings.host, settings.flow);
                            
                            // Run sync in separate thread with its own tokio runtime
                            std::thread::spawn(move || {
                                let rt = tokio::runtime::Runtime::new().unwrap();
                                rt.block_on(async {
                                    if let Err(e) = perform_sync(&settings).await {
                                        error!("Sync failed: {}", e);
                                    } else {
                                        info!("Sync completed successfully");
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
                *self.settings.lock().unwrap() = new_settings;
                self.update_menu();
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
