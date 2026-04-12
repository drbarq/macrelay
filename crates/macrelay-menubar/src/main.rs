mod config;
mod launchagent;
mod process;
mod uninstall;

use std::collections::HashMap;
use std::process::Command;
use std::time::{Duration, Instant};

use config::{MenuBarConfig, SERVICES};
use launchagent::{disable_launch_at_login, enable_launch_at_login, is_launch_at_login_enabled};
use muda::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use process::{is_macrelay_running, status_text};
use tao::event::{Event, StartCause};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tao::platform::macos::{ActivationPolicy, EventLoopExtMacOS};
use tray_icon::TrayIconBuilder;

const REFRESH_INTERVAL: Duration = Duration::from_secs(5);

/// Permission entries for the menu. Each maps to a System Settings pane.
const PERMISSION_ENTRIES: &[PermissionEntry] = &[
    PermissionEntry {
        label: "Accessibility — UI automation tools",
        pane: "Privacy_Accessibility",
    },
    PermissionEntry {
        label: "Full Disk Access — Messages search",
        pane: "Privacy_AllFiles",
    },
    PermissionEntry {
        label: "Calendars — Calendar tools",
        pane: "Privacy_Calendars",
    },
    PermissionEntry {
        label: "Reminders — Reminder tools",
        pane: "Privacy_Reminders",
    },
    PermissionEntry {
        label: "Contacts — Contact tools",
        pane: "Privacy_Contacts",
    },
    PermissionEntry {
        label: "Location Services — Location tool",
        pane: "Privacy_LocationServices",
    },
    PermissionEntry {
        label: "Automation — Mail, Notes, Stickies",
        pane: "Privacy_Automation",
    },
];

struct PermissionEntry {
    label: &'static str,
    pane: &'static str,
}

/// Open System Settings to a specific Privacy & Security pane.
fn open_settings_pane(pane: &str) {
    let url = format!("x-apple.systempreferences:com.apple.preference.security?{pane}");
    let _ = Command::new("open").arg(&url).spawn();
}

/// Load the menu bar icon from the embedded PNG asset.
fn load_icon() -> tray_icon::Icon {
    let png_bytes = include_bytes!("../../../assets/menubar_v3_36.png");
    let img = image::load_from_memory(png_bytes).expect("Failed to decode menu bar icon");
    let rgba = img.into_rgba8();
    let (w, h) = (rgba.width(), rgba.height());
    tray_icon::Icon::from_rgba(rgba.into_raw(), w, h).expect("Failed to create tray icon")
}

fn main() {
    let mut event_loop = EventLoopBuilder::new().build();
    event_loop.set_activation_policy(ActivationPolicy::Accessory);
    let mut app_config = MenuBarConfig::load();

    // Build the menu
    let menu = Menu::new();

    // Title (disabled, just a label)
    let title_item = MenuItem::new("MacRelay", false, None);
    menu.append(&title_item).unwrap();

    // Status indicator
    let running = is_macrelay_running();
    let status_item = MenuItem::new(status_text(running), false, None);
    menu.append(&status_item).unwrap();

    menu.append(&PredefinedMenuItem::separator()).unwrap();

    // Service toggle items
    let mut service_items: HashMap<muda::MenuId, &str> = HashMap::new();
    for svc in SERVICES {
        let checked = app_config.is_enabled(svc.key);
        let item = CheckMenuItem::new(svc.label, true, checked, None);
        service_items.insert(item.id().clone(), svc.key);
        menu.append(&item).unwrap();
    }

    // Restart hint (always visible as a reminder)
    let restart_hint = MenuItem::new("  ↳ Restart client to apply changes", false, None);
    menu.append(&restart_hint).unwrap();

    menu.append(&PredefinedMenuItem::separator()).unwrap();

    // Clients submenu — choose which clients get configured
    let clients_submenu = Submenu::new("Clients", true);
    let claude_desktop_item = CheckMenuItem::new(
        "Claude Desktop",
        true,
        app_config.claude_desktop_enabled,
        None,
    );
    let claude_desktop_id = claude_desktop_item.id().clone();
    clients_submenu.append(&claude_desktop_item).unwrap();

    let claude_code_item =
        CheckMenuItem::new("Claude Code", true, app_config.claude_code_enabled, None);
    let claude_code_id = claude_code_item.id().clone();
    clients_submenu.append(&claude_code_item).unwrap();
    menu.append(&clients_submenu).unwrap();

    // Permissions submenu — guide to required permissions, click to open System Settings
    let permissions_submenu = Submenu::new("Permissions", true);
    let hint = MenuItem::new("  Grant in System Settings:", false, None);
    permissions_submenu.append(&hint).unwrap();
    let mut perm_ids: HashMap<muda::MenuId, &str> = HashMap::new();
    for entry in PERMISSION_ENTRIES {
        let item = MenuItem::new(entry.label, true, None);
        perm_ids.insert(item.id().clone(), entry.pane);
        permissions_submenu.append(&item).unwrap();
    }
    menu.append(&permissions_submenu).unwrap();

    menu.append(&PredefinedMenuItem::separator()).unwrap();

    // Launch at Login toggle
    let launch_at_login_item =
        CheckMenuItem::new("Launch at Login", true, is_launch_at_login_enabled(), None);
    let launch_at_login_id = launch_at_login_item.id().clone();
    menu.append(&launch_at_login_item).unwrap();

    // Uninstall
    let uninstall_item = MenuItem::new("Uninstall MacRelay...", true, None);
    let uninstall_id = uninstall_item.id().clone();
    menu.append(&uninstall_item).unwrap();

    // Quit
    let quit_item = MenuItem::new("Quit", true, None);
    let quit_id = quit_item.id().clone();
    menu.append(&quit_item).unwrap();

    // Create tray icon
    let _tray = TrayIconBuilder::new()
        .with_icon(load_icon())
        .with_icon_as_template(true)
        .with_menu(Box::new(menu))
        .with_tooltip("MacRelay")
        .build()
        .expect("Failed to create tray icon");

    let menu_channel = MenuEvent::receiver();

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::WaitUntil(Instant::now() + REFRESH_INTERVAL);

        match event {
            Event::NewEvents(StartCause::Init) => {
                app_config.write_client_configs();
            }
            Event::NewEvents(StartCause::ResumeTimeReached { .. }) => {
                let running = is_macrelay_running();
                status_item.set_text(status_text(running));
            }
            _ => {}
        }

        if let Ok(menu_event) = menu_channel.try_recv() {
            if menu_event.id == quit_id {
                *control_flow = ControlFlow::Exit;
                return;
            }

            if menu_event.id == uninstall_id {
                let actions = uninstall::uninstall();
                eprintln!("MacRelay uninstalled:");
                for action in &actions {
                    eprintln!("  {action}");
                }
                *control_flow = ControlFlow::Exit;
                return;
            }

            if menu_event.id == launch_at_login_id {
                if is_launch_at_login_enabled() {
                    disable_launch_at_login();
                } else {
                    enable_launch_at_login();
                }
                return;
            }

            // Client toggles
            if menu_event.id == claude_desktop_id {
                app_config.claude_desktop_enabled = !app_config.claude_desktop_enabled;
                app_config.save();
                app_config.write_client_configs();
                return;
            }
            if menu_event.id == claude_code_id {
                app_config.claude_code_enabled = !app_config.claude_code_enabled;
                app_config.save();
                app_config.write_client_configs();
                return;
            }

            // Permission items — open System Settings to the right pane
            if let Some(pane) = perm_ids.get(&menu_event.id) {
                open_settings_pane(pane);
                return;
            }

            // Service toggles
            if let Some(key) = service_items.get(&menu_event.id) {
                app_config.toggle(key);
                app_config.write_client_configs();
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permission_entries_have_labels_and_panes() {
        assert!(!PERMISSION_ENTRIES.is_empty());
        for entry in PERMISSION_ENTRIES {
            assert!(!entry.label.is_empty());
            assert!(entry.pane.starts_with("Privacy_"));
        }
    }

    #[test]
    fn permission_entries_cover_key_categories() {
        let labels: Vec<&str> = PERMISSION_ENTRIES.iter().map(|e| e.label).collect();
        let all = labels.join(" ");
        assert!(all.contains("Accessibility"));
        assert!(all.contains("Full Disk Access"));
        assert!(all.contains("Calendar"));
        assert!(all.contains("Reminders"));
        assert!(all.contains("Contacts"));
        assert!(all.contains("Location"));
        assert!(all.contains("Automation"));
    }
}
