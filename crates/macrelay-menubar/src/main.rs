mod config;
mod launchagent;
mod process;
mod uninstall;

use std::collections::HashMap;
use std::process::Command;
use std::time::{Duration, Instant};

use config::{MenuBarConfig, SERVICES};
use launchagent::{disable_launch_at_login, enable_launch_at_login, is_launch_at_login_enabled};
use macrelay_core::permissions::{PermissionManager, PermissionStatus, PermissionType};
use muda::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use process::{is_macrelay_running, status_text};
use tao::event::{Event, StartCause};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tao::platform::macos::{ActivationPolicy, EventLoopExtMacOS};
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

/// Open System Settings to the Privacy & Security pane for a permission type.
fn open_permission_settings(perm_type: PermissionType) {
    let pane = match perm_type {
        PermissionType::Accessibility => "Privacy_Accessibility",
        PermissionType::ScreenRecording => "Privacy_ScreenCapture",
        PermissionType::FullDiskAccess => "Privacy_AllFiles",
        PermissionType::Calendar => "Privacy_Calendars",
        PermissionType::Reminders => "Privacy_Reminders",
        PermissionType::Contacts => "Privacy_Contacts",
        PermissionType::Location => "Privacy_LocationServices",
    };
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

    // Permissions submenu with live status (clickable to open System Settings)
    let permissions_submenu = Submenu::new("Permissions", true);
    let mut perm_items: Vec<MenuItem> = Vec::new();
    let mut perm_ids: HashMap<muda::MenuId, PermissionType> = HashMap::new();
    for perm_type in &PERMISSION_ORDER {
        let label = format_permission_label(*perm_type, PermissionStatus::Unknown);
        let item = MenuItem::new(&label, true, None);
        perm_ids.insert(item.id().clone(), *perm_type);
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

            // Permission items — open System Settings
            if let Some(perm_type) = perm_ids.get(&menu_event.id) {
                open_permission_settings(*perm_type);
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
