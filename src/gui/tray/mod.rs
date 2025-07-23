mod app;
mod icon;

pub use app::*;
pub use icon::{
    create_tooltip, create_tray_icon, start_auto_sync_task, update_sync_status, update_tray_menu,
    SyncStatus, TrayMenuIds,
};
