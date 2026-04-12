mod config;
mod launchagent;
mod process;

use std::collections::HashMap;
use std::time::{Duration, Instant};

use config::{MenuBarConfig, SERVICES};
use launchagent::{disable_launch_at_login, enable_launch_at_login, is_launch_at_login_enabled};
use macrelay_core::permissions::{PermissionManager, PermissionStatus, PermissionType};
use muda::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use process::{is_macrelay_running, status_text};
use tao::event::{Event, StartCause};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tray_icon::TrayIconBuilder;

const REFRESH_INTERVAL: Duration = Duration::from_secs(5);

const PERMISSION_ORDER: [PermissionType; 7] = [
    PermissionType::Accessibility,
    PermissionType::ScreenRecording,
    PermissionType::FullDiskAccess,
    PermissionType::Calendar,
    PermissionType::Reminders,
    PermissionType::Contacts,
    PermissionType::Location,
];

fn format_permission_label(perm_type: PermissionType, status: PermissionStatus) -> String {
    let icon = match status {
        PermissionStatus::Granted => "✓",
        PermissionStatus::Denied => "✗",
        PermissionStatus::NotDetermined | PermissionStatus::Unknown => "?",
    };
    format!("{icon}  {perm_type}")
}

fn refresh_permissions(perm_items: &[MenuItem]) {
    let statuses = PermissionManager::check_all();
    for (i, perm_type) in PERMISSION_ORDER.iter().enumerate() {
        let status = statuses
            .get(perm_type)
            .copied()
            .unwrap_or(PermissionStatus::Unknown);
        perm_items[i].set_text(format_permission_label(*perm_type, status));
    }
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
    let event_loop = EventLoopBuilder::new().build();
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

    menu.append(&PredefinedMenuItem::separator()).unwrap();

    // Permissions submenu with live status (refreshed on timer)
    let permissions_submenu = Submenu::new("Permissions", true);
    let mut perm_items: Vec<MenuItem> = Vec::new();
    for perm_type in &PERMISSION_ORDER {
        let label = format_permission_label(*perm_type, PermissionStatus::Unknown);
        let item = MenuItem::new(&label, false, None);
        permissions_submenu.append(&item).unwrap();
        perm_items.push(item);
    }
    refresh_permissions(&perm_items);
    menu.append(&permissions_submenu).unwrap();

    menu.append(&PredefinedMenuItem::separator()).unwrap();

    // Launch at Login toggle
    let launch_at_login_item =
        CheckMenuItem::new("Launch at Login", true, is_launch_at_login_enabled(), None);
    let launch_at_login_id = launch_at_login_item.id().clone();
    menu.append(&launch_at_login_item).unwrap();

    // Refresh Config (writes config and updates status display)
    let refresh_item = MenuItem::new("Refresh Config", true, None);
    let refresh_id = refresh_item.id().clone();
    menu.append(&refresh_item).unwrap();

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
                // Write initial config on first launch
                app_config.write_claude_desktop_config();
            }
            Event::NewEvents(StartCause::ResumeTimeReached { .. }) => {
                // Periodic refresh: update permissions and server status
                refresh_permissions(&perm_items);
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

            if menu_event.id == launch_at_login_id {
                if is_launch_at_login_enabled() {
                    disable_launch_at_login();
                } else {
                    enable_launch_at_login();
                }
                return;
            }

            if menu_event.id == refresh_id {
                // Re-write config and update status display.
                // Claude Desktop manages the server lifecycle; this ensures
                // the config file reflects current toggle state.
                app_config.write_claude_desktop_config();
                refresh_permissions(&perm_items);
                let running = is_macrelay_running();
                status_item.set_text(status_text(running));
                return;
            }

            // Check if it's a service toggle
            if let Some(key) = service_items.get(&menu_event.id) {
                app_config.toggle(key);
                app_config.write_claude_desktop_config();
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_permission_label_granted() {
        let label =
            format_permission_label(PermissionType::Accessibility, PermissionStatus::Granted);
        assert!(label.starts_with("✓"));
        assert!(label.contains("Accessibility"));
    }

    #[test]
    fn format_permission_label_denied() {
        let label = format_permission_label(PermissionType::Calendar, PermissionStatus::Denied);
        assert!(label.starts_with("✗"));
        assert!(label.contains("Calendar"));
    }

    #[test]
    fn format_permission_label_unknown() {
        let label =
            format_permission_label(PermissionType::Location, PermissionStatus::NotDetermined);
        assert!(label.starts_with("?"));
        assert!(label.contains("Location"));
    }

    #[test]
    fn permission_order_covers_all_types() {
        assert_eq!(PERMISSION_ORDER.len(), 7);
    }
}
