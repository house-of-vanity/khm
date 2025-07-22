#![allow(unexpected_cfgs)]
use super::{load_settings, save_settings, KhmSettings};
use cocoa::appkit::*;
use cocoa::base::{id, nil, NO, YES};
use cocoa::foundation::{NSAutoreleasePool, NSPoint, NSRect, NSSize, NSString, NSDefaultRunLoopMode};
use log::{debug, error, info};
use objc::{msg_send, sel, sel_impl};
use std::ffi::CStr;

// NSTextFieldBezelStyle constants
#[allow(non_upper_case_globals)]
const NSTextFieldSquareBezel: u32 = 0;

const WINDOW_WIDTH: f64 = 450.0;
const WINDOW_HEIGHT: f64 = 385.0;
const MARGIN: f64 = 20.0;
const FIELD_HEIGHT: f64 = 24.0;
const BUTTON_HEIGHT: f64 = 32.0;

// NSControl state constants
const NS_CONTROL_STATE_VALUE_OFF: i32 = 0;
const NS_CONTROL_STATE_VALUE_ON: i32 = 1;

// NSButton type constants
const NS_SWITCH_BUTTON: u32 = 3;

struct MacOSKhmSettingsWindow {
    window: id,
    host_field: id,
    flow_field: id,
    known_hosts_field: id,
    basic_auth_field: id,
    auto_sync_field: id,
    in_place_checkbox: id,
}

impl MacOSKhmSettingsWindow {
    fn new() -> Self {
        info!("Creating macOS KHM settings window");
        unsafe {
            let settings = load_settings();
            info!("KHM Settings loaded: host={}, flow={}", settings.host, settings.flow);
            
            // Create window
            let window: id = msg_send![NSWindow::alloc(nil), 
                initWithContentRect: NSRect::new(
                    NSPoint::new(100.0, 100.0),
                    NSSize::new(WINDOW_WIDTH, WINDOW_HEIGHT),
                )
                styleMask: NSWindowStyleMask::NSTitledWindowMask | NSWindowStyleMask::NSClosableWindowMask | NSWindowStyleMask::NSMiniaturizableWindowMask
                backing: NSBackingStoreType::NSBackingStoreBuffered
                defer: NO
            ];
            info!("Window allocated and initialized");
            
            let _: () = msg_send![window, setTitle: NSString::alloc(nil).init_str("KHM Settings")];
            let _: () = msg_send![window, center];
            let _: () = msg_send![window, setReleasedWhenClosed: NO];
            
            let content_view: id = msg_send![window, contentView];
            
            let mut current_y = WINDOW_HEIGHT - MARGIN - 30.0;
            
            // Host label and field
            let host_label: id = msg_send![NSTextField::alloc(nil),
                initWithFrame: NSRect::new(
                    NSPoint::new(MARGIN, current_y),
                    NSSize::new(100.0, 20.0),
                )
            ];
            let _: () = msg_send![host_label, setStringValue: NSString::alloc(nil).init_str("Host URL:")];
            let _: () = msg_send![host_label, setBezeled: NO];
            let _: () = msg_send![host_label, setDrawsBackground: NO];
            let _: () = msg_send![host_label, setEditable: NO];
            let _: () = msg_send![host_label, setSelectable: NO];
            let _: () = msg_send![content_view, addSubview: host_label];
            
            let host_field: id = msg_send![NSTextField::alloc(nil),
                initWithFrame: NSRect::new(
                    NSPoint::new(MARGIN + 110.0, current_y),
                    NSSize::new(310.0, FIELD_HEIGHT),
                )
            ];
            let _: () = msg_send![host_field, setStringValue: NSString::alloc(nil).init_str(&settings.host)];
            let _: () = msg_send![host_field, setEditable: YES];
            let _: () = msg_send![host_field, setSelectable: YES];
            let _: () = msg_send![host_field, setBezeled: YES];
            let _: () = msg_send![host_field, setBezelStyle: NSTextFieldSquareBezel];
            let _: () = msg_send![content_view, addSubview: host_field];
            
            current_y -= 35.0;
            
            // Flow label and field
            let flow_label: id = msg_send![NSTextField::alloc(nil),
                initWithFrame: NSRect::new(
                    NSPoint::new(MARGIN, current_y),
                    NSSize::new(100.0, 20.0),
                )
            ];
            let _: () = msg_send![flow_label, setStringValue: NSString::alloc(nil).init_str("Flow Name:")];
            let _: () = msg_send![flow_label, setBezeled: NO];
            let _: () = msg_send![flow_label, setDrawsBackground: NO];
            let _: () = msg_send![flow_label, setEditable: NO];
            let _: () = msg_send![flow_label, setSelectable: NO];
            let _: () = msg_send![content_view, addSubview: flow_label];
            
            let flow_field: id = msg_send![NSTextField::alloc(nil),
                initWithFrame: NSRect::new(
                    NSPoint::new(MARGIN + 110.0, current_y),
                    NSSize::new(310.0, FIELD_HEIGHT),
                )
            ];
            let _: () = msg_send![flow_field, setStringValue: NSString::alloc(nil).init_str(&settings.flow)];
            let _: () = msg_send![flow_field, setEditable: YES];
            let _: () = msg_send![flow_field, setSelectable: YES];
            let _: () = msg_send![flow_field, setBezeled: YES];
            let _: () = msg_send![flow_field, setBezelStyle: NSTextFieldSquareBezel];
            let _: () = msg_send![content_view, addSubview: flow_field];
            
            current_y -= 35.0;
            
            // Known hosts label and field
            let known_hosts_label: id = msg_send![NSTextField::alloc(nil),
                initWithFrame: NSRect::new(
                    NSPoint::new(MARGIN, current_y),
                    NSSize::new(100.0, 20.0),
                )
            ];
            let _: () = msg_send![known_hosts_label, setStringValue: NSString::alloc(nil).init_str("Known Hosts:")];
            let _: () = msg_send![known_hosts_label, setBezeled: NO];
            let _: () = msg_send![known_hosts_label, setDrawsBackground: NO];
            let _: () = msg_send![known_hosts_label, setEditable: NO];
            let _: () = msg_send![known_hosts_label, setSelectable: NO];
            let _: () = msg_send![content_view, addSubview: known_hosts_label];
            
            let known_hosts_field: id = msg_send![NSTextField::alloc(nil),
                initWithFrame: NSRect::new(
                    NSPoint::new(MARGIN + 110.0, current_y),
                    NSSize::new(310.0, FIELD_HEIGHT),
                )
            ];
            let _: () = msg_send![known_hosts_field, setStringValue: NSString::alloc(nil).init_str(&settings.known_hosts)];
            let _: () = msg_send![known_hosts_field, setEditable: YES];
            let _: () = msg_send![known_hosts_field, setSelectable: YES];
            let _: () = msg_send![known_hosts_field, setBezeled: YES];
            let _: () = msg_send![known_hosts_field, setBezelStyle: NSTextFieldSquareBezel];
            let _: () = msg_send![content_view, addSubview: known_hosts_field];
            
            current_y -= 35.0;
            
            // Basic auth label and field
            let basic_auth_label: id = msg_send![NSTextField::alloc(nil),
                initWithFrame: NSRect::new(
                    NSPoint::new(MARGIN, current_y),
                    NSSize::new(100.0, 20.0),
                )
            ];
            let _: () = msg_send![basic_auth_label, setStringValue: NSString::alloc(nil).init_str("Basic Auth:")];
            let _: () = msg_send![basic_auth_label, setBezeled: NO];
            let _: () = msg_send![basic_auth_label, setDrawsBackground: NO];
            let _: () = msg_send![basic_auth_label, setEditable: NO];
            let _: () = msg_send![basic_auth_label, setSelectable: NO];
            let _: () = msg_send![content_view, addSubview: basic_auth_label];
            
            let basic_auth_field: id = msg_send![NSTextField::alloc(nil),
                initWithFrame: NSRect::new(
                    NSPoint::new(MARGIN + 110.0, current_y),
                    NSSize::new(310.0, FIELD_HEIGHT),
                )
            ];
            let _: () = msg_send![basic_auth_field, setStringValue: NSString::alloc(nil).init_str(&settings.basic_auth)];
            let _: () = msg_send![basic_auth_field, setEditable: YES];
            let _: () = msg_send![basic_auth_field, setSelectable: YES];
            let _: () = msg_send![basic_auth_field, setBezeled: YES];
            let _: () = msg_send![basic_auth_field, setBezelStyle: NSTextFieldSquareBezel];
            let _: () = msg_send![content_view, addSubview: basic_auth_field];
            
            current_y -= 35.0;
            
            // Auto sync interval label and field
            let auto_sync_label: id = msg_send![NSTextField::alloc(nil),
                initWithFrame: NSRect::new(
                    NSPoint::new(MARGIN, current_y),
                    NSSize::new(100.0, 20.0),
                )
            ];
            let _: () = msg_send![auto_sync_label, setStringValue: NSString::alloc(nil).init_str("Auto sync (min):")];
            let _: () = msg_send![auto_sync_label, setBezeled: NO];
            let _: () = msg_send![auto_sync_label, setDrawsBackground: NO];
            let _: () = msg_send![auto_sync_label, setEditable: NO];
            let _: () = msg_send![auto_sync_label, setSelectable: NO];
            let _: () = msg_send![content_view, addSubview: auto_sync_label];
            
            let auto_sync_field: id = msg_send![NSTextField::alloc(nil),
                initWithFrame: NSRect::new(
                    NSPoint::new(MARGIN + 110.0, current_y),
                    NSSize::new(310.0, FIELD_HEIGHT),
                )
            ];
            let _: () = msg_send![auto_sync_field, setStringValue: NSString::alloc(nil).init_str(&settings.auto_sync_interval_minutes.to_string())];
            let _: () = msg_send![auto_sync_field, setEditable: YES];
            let _: () = msg_send![auto_sync_field, setSelectable: YES];
            let _: () = msg_send![auto_sync_field, setBezeled: YES];
            let _: () = msg_send![auto_sync_field, setBezelStyle: NSTextFieldSquareBezel];
            let _: () = msg_send![content_view, addSubview: auto_sync_field];
            
            current_y -= 40.0;
            
            // In place checkbox
            let in_place_checkbox: id = msg_send![NSButton::alloc(nil),
                initWithFrame: NSRect::new(
                    NSPoint::new(MARGIN, current_y),
                    NSSize::new(400.0, 24.0),
                )
            ];
            let _: () = msg_send![in_place_checkbox, setButtonType: NS_SWITCH_BUTTON];
            let _: () = msg_send![in_place_checkbox, setTitle: NSString::alloc(nil).init_str("Update known_hosts file in-place after sync")];
            let _: () = msg_send![in_place_checkbox, setState: if settings.in_place { NS_CONTROL_STATE_VALUE_ON } else { NS_CONTROL_STATE_VALUE_OFF }];
            let _: () = msg_send![content_view, addSubview: in_place_checkbox];
            
            // Save button
            let save_button: id = msg_send![NSButton::alloc(nil),
                initWithFrame: NSRect::new(
                    NSPoint::new(WINDOW_WIDTH - 180.0, MARGIN),
                    NSSize::new(80.0, BUTTON_HEIGHT),
                )
            ];
            let _: () = msg_send![save_button, setTitle: NSString::alloc(nil).init_str("Save")];
            let _: () = msg_send![content_view, addSubview: save_button];
            
            // Cancel button
            let cancel_button: id = msg_send![NSButton::alloc(nil),
                initWithFrame: NSRect::new(
                    NSPoint::new(WINDOW_WIDTH - 90.0, MARGIN),
                    NSSize::new(80.0, BUTTON_HEIGHT),
                )
            ];
            let _: () = msg_send![cancel_button, setTitle: NSString::alloc(nil).init_str("Cancel")];
            let _: () = msg_send![content_view, addSubview: cancel_button];
            
            info!("All KHM UI elements created successfully");
            
            Self {
                window,
                host_field,
                flow_field,
                known_hosts_field,
                basic_auth_field,
                auto_sync_field,
                in_place_checkbox,
            }
        }
    }
    
    fn collect_settings(&self) -> KhmSettings {
        unsafe {
            // Get host
            let host_ns_string: id = msg_send![self.host_field, stringValue];
            let host_ptr: *const i8 = msg_send![host_ns_string, UTF8String];
            let host = CStr::from_ptr(host_ptr).to_string_lossy().to_string();
            
            // Get flow
            let flow_ns_string: id = msg_send![self.flow_field, stringValue];
            let flow_ptr: *const i8 = msg_send![flow_ns_string, UTF8String];
            let flow = CStr::from_ptr(flow_ptr).to_string_lossy().to_string();
            
            // Get known hosts path
            let known_hosts_ns_string: id = msg_send![self.known_hosts_field, stringValue];
            let known_hosts_ptr: *const i8 = msg_send![known_hosts_ns_string, UTF8String];
            let known_hosts = CStr::from_ptr(known_hosts_ptr).to_string_lossy().to_string();
            
            // Get basic auth
            let basic_auth_ns_string: id = msg_send![self.basic_auth_field, stringValue];
            let basic_auth_ptr: *const i8 = msg_send![basic_auth_ns_string, UTF8String];
            let basic_auth = CStr::from_ptr(basic_auth_ptr).to_string_lossy().to_string();
            
            // Get auto sync interval
            let auto_sync_ns_string: id = msg_send![self.auto_sync_field, stringValue];
            let auto_sync_ptr: *const i8 = msg_send![auto_sync_ns_string, UTF8String];
            let auto_sync_str = CStr::from_ptr(auto_sync_ptr).to_string_lossy().to_string();
            let auto_sync_interval_minutes = auto_sync_str.parse::<u32>().unwrap_or(60); // Default to 60 if parse fails
            
            // Get checkbox state
            let in_place_state: i32 = msg_send![self.in_place_checkbox, state];
            let in_place = in_place_state == NS_CONTROL_STATE_VALUE_ON;
            
            KhmSettings {
                host,
                flow,
                known_hosts,
                basic_auth,
                in_place,
                auto_sync_interval_minutes,
            }
        }
    }
}

pub fn run_settings_window() {
    info!("Starting native macOS KHM settings window");
    unsafe {
        let pool = NSAutoreleasePool::new(nil);
        let app = NSApp();
        info!("NSApp created for settings window");
        
        // Set activation policy to regular for this standalone window
        let _: () = msg_send![app, setActivationPolicy: 0]; // NSApplicationActivationPolicyRegular
        info!("Activation policy set to Regular for settings window");
        
        let settings_window = MacOSKhmSettingsWindow::new();
        let window = settings_window.window;
        info!("KHM settings window created");
        
        // Show window and activate app
        let _: () = msg_send![app, activateIgnoringOtherApps: YES];
        let _: () = msg_send![window, makeKeyAndOrderFront: nil];
        let _: () = msg_send![window, orderFrontRegardless];
        info!("Settings window should be visible now");
        
        // Run event loop until window is closed
        let mut should_close = false;
        while !should_close {
            let event: id = msg_send![app,
                nextEventMatchingMask: NSEventMask::NSAnyEventMask.bits()
                untilDate: nil
                inMode: NSDefaultRunLoopMode
                dequeue: YES
            ];
            
            if event == nil {
                continue;
            }
            
            let event_type: NSEventType = msg_send![event, type];
            
            // Handle window close button
            if event_type == NSEventType::NSLeftMouseDown {
                let event_window: id = msg_send![event, window];
                if event_window == window {
                    let location: NSPoint = msg_send![event, locationInWindow];
                    
                    // Check if click is on Save button
                    if location.x >= WINDOW_WIDTH - 180.0 && location.x <= WINDOW_WIDTH - 100.0 &&
                       location.y >= MARGIN && location.y <= MARGIN + BUTTON_HEIGHT {
                        info!("Save button clicked");
                        let settings = settings_window.collect_settings();
                        if let Err(e) = save_settings(&settings) {
                            error!("Failed to save KHM settings: {}", e);
                        } else {
                            info!("KHM settings saved from native macOS window");
                        }
                        should_close = true;
                        continue;
                    }
                    
                    // Check if click is on Cancel button
                    if location.x >= WINDOW_WIDTH - 90.0 && location.x <= WINDOW_WIDTH - 10.0 &&
                       location.y >= MARGIN && location.y <= MARGIN + BUTTON_HEIGHT {
                        info!("Cancel button clicked");
                        should_close = true;
                        continue;
                    }
                }
            }
            
            // Check if window is closed via close button or ESC
            if event_type == NSEventType::NSKeyDown {
                let key_code: u16 = msg_send![event, keyCode];
                let flags: NSEventModifierFlags = msg_send![event, modifierFlags];
                
                // Handle Cmd+V (paste), Cmd+C (copy), Cmd+X (cut), Cmd+A (select all)
                if flags.contains(NSEventModifierFlags::NSCommandKeyMask) {
                    match key_code {
                        9 => { // V key - paste
                            let responder: id = msg_send![window, firstResponder];
                            let _: () = msg_send![responder, paste: nil];
                            continue;
                        }
                        8 => { // C key - copy
                            let responder: id = msg_send![window, firstResponder];
                            let _: () = msg_send![responder, copy: nil];
                            continue;
                        }
                        7 => { // X key - cut
                            let responder: id = msg_send![window, firstResponder];
                            let _: () = msg_send![responder, cut: nil];
                            continue;
                        }
                        0 => { // A key - select all
                            let responder: id = msg_send![window, firstResponder];
                            let _: () = msg_send![responder, selectAll: nil];
                            continue;
                        }
                        _ => {}
                    }
                }
                
                if key_code == 53 { // ESC key
                    info!("ESC pressed, closing settings window");
                    should_close = true;
                    continue;
                }
            }
            
            // Forward event to application
            let _: () = msg_send![app, sendEvent: event];
        }
        
        let _: () = msg_send![window, close];
        info!("Native macOS KHM settings window closed");
        
        pool.drain();
    }
}
