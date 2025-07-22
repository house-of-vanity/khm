use log::{error, info};
use std::sync::{Arc, Mutex};
use tray_icon::{
    menu::{Menu, MenuItem, MenuId},
    TrayIcon, TrayIconBuilder,
};
use crate::gui::common::{KhmSettings, perform_sync};

#[derive(Debug, Clone)]
pub struct SyncStatus {
    pub last_sync_time: Option<std::time::Instant>,
    pub last_sync_keys: Option<usize>,
    pub next_sync_in_seconds: Option<u64>,
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

pub struct TrayMenuIds {
    pub settings_id: MenuId,
    pub quit_id: MenuId,
    pub sync_id: MenuId,
}

/// Create tray icon with menu
pub fn create_tray_icon(settings: &KhmSettings, sync_status: &SyncStatus) -> (TrayIcon, TrayMenuIds) {
    // Create simple blue icon
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
    let tooltip = create_tooltip(settings, sync_status);
    
    let tray_icon = TrayIconBuilder::new()
        .with_tooltip(&tooltip)
        .with_icon(icon)
        .with_menu(Box::new(menu))
        .build()
        .unwrap();
    
    let menu_ids = TrayMenuIds {
        settings_id,
        quit_id,
        sync_id,
    };
    
    (tray_icon, menu_ids)
}

/// Update tray menu with new settings
pub fn update_tray_menu(tray_icon: &TrayIcon, settings: &KhmSettings) -> TrayMenuIds {
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
    
    tray_icon.set_menu(Some(Box::new(menu)));
    
    TrayMenuIds {
        settings_id,
        quit_id,
        sync_id,
    }
}

/// Create tooltip text for tray icon
pub fn create_tooltip(settings: &KhmSettings, sync_status: &SyncStatus) -> String {
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
    
    tooltip
}

/// Start auto sync background task
pub fn start_auto_sync_task(
    settings: Arc<Mutex<KhmSettings>>,
    sync_status: Arc<Mutex<SyncStatus>>,
    event_sender: winit::event_loop::EventLoopProxy<crate::gui::UserEvent>
) -> Option<std::thread::JoinHandle<()>> {
    let initial_settings = settings.lock().unwrap().clone();
    
    // Only start auto sync if settings are valid and in_place is enabled
    if initial_settings.host.is_empty() || initial_settings.flow.is_empty() || !initial_settings.in_place {
        info!("Auto sync disabled or settings invalid");
        return None;
    }
    
    info!("Starting auto sync with interval {} minutes", initial_settings.auto_sync_interval_minutes);
    
    let handle = std::thread::spawn(move || {
        // Initial sync on startup
        info!("Performing initial sync on startup");
        let current_settings = settings.lock().unwrap().clone();
        if !current_settings.host.is_empty() && !current_settings.flow.is_empty() {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                match perform_sync(&current_settings).await {
                    Ok(keys_count) => {
                        info!("Initial sync completed successfully with {} keys", keys_count);
                        let mut status = sync_status.lock().unwrap();
                        status.last_sync_time = Some(std::time::Instant::now());
                        status.last_sync_keys = Some(keys_count);
                        let _ = event_sender.send_event(crate::gui::UserEvent::UpdateMenu);
                    }
                    Err(e) => {
                        error!("Initial sync failed: {}", e);
                    }
                }
            });
        }
        
        // Start menu update timer
        let timer_sender = event_sender.clone();
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(std::time::Duration::from_secs(1));
                let _ = timer_sender.send_event(crate::gui::UserEvent::UpdateMenu);
            }
        });
        
        // Periodic sync
        loop {
            let interval_minutes = current_settings.auto_sync_interval_minutes;
            std::thread::sleep(std::time::Duration::from_secs(interval_minutes as u64 * 60));
            
            let current_settings = settings.lock().unwrap().clone();
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
                        let mut status = sync_status.lock().unwrap();
                        status.last_sync_time = Some(std::time::Instant::now());
                        status.last_sync_keys = Some(keys_count);
                        let _ = event_sender.send_event(crate::gui::UserEvent::UpdateMenu);
                    }
                    Err(e) => {
                        error!("Auto sync failed: {}", e);
                    }
                }
            });
        }
    });
    
    Some(handle)
}

/// Update sync status for tooltip
pub fn update_sync_status(settings: &KhmSettings, sync_status: &mut SyncStatus) {
    if !settings.host.is_empty() && !settings.flow.is_empty() && settings.in_place {
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
    }
}
