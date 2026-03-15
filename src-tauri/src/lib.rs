mod commands;
mod error;
mod logic;
mod types;

use commands::{get_logs, list_namespaces, list_services, open_url, ping_service};
use tauri::Manager;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri_plugin_positioner::{Position, WindowExt};

pub struct AppState {
    pub kube_client: kube::Client,
    pub http_client: reqwest::Client,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("failed to install rustls crypto provider");

    tauri::Builder::default()
        .plugin(tauri_plugin_positioner::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let kube_client = tauri::async_runtime::block_on(kube::Client::try_default())
                .map_err(|e| format!("kubeconfig error: {e}"))?;
            let http_client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(60))
                .build()
                .map_err(|e| format!("http client error: {e}"))?;
            app.manage(AppState { kube_client, http_client });

            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let quit = MenuItem::with_id(app, "quit", "Exit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&quit])?;

            let icon = tauri::include_image!("icons/32x32.png");

            TrayIconBuilder::new()
                .icon(icon)
                .icon_as_template(true)
                .tooltip("Knative Explorer")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_tray_icon_event(|tray, event| {
                    // Let the positioner plugin record the tray icon position before we use it.
                    tauri_plugin_positioner::on_tray_event(tray.app_handle(), &event);

                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            if window.is_visible().unwrap_or(false) {
                                let _ = window.hide();
                            } else {
                                let _ = window.move_window(Position::TrayCenter);
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                    }
                })
                .on_menu_event(|app, event| {
                    if event.id().as_ref() == "quit" {
                        app.exit(0);
                    }
                })
                .build(app)?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_namespaces,
            list_services,
            ping_service,
            open_url,
            get_logs,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
