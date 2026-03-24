mod commands;
mod error;
mod logic;
mod types;
mod watcher;

use commands::{
    fetch_one_service, list_namespaces, list_pods, list_services, open_url, pause_log_stream,
    ping_service, resume_log_stream, stop_log_stream, stream_logs,
};
use logic::LogStreamHandle;
use std::sync::Mutex;
use tauri::Manager;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{PhysicalPosition};

pub struct AppState {
    pub kube_client: kube::Client,
    pub http_client: reqwest::Client,
    pub watched_ns: std::sync::Arc<tokio::sync::RwLock<String>>,
    pub log_stream: std::sync::Arc<Mutex<Option<LogStreamHandle>>>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("failed to install rustls crypto provider");

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let kube_client = tauri::async_runtime::block_on(kube::Client::try_default())
                .map_err(|e| format!("kubeconfig error: {e}"))?;
            let http_client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(60))
                .build()
                .map_err(|e| format!("http client error: {e}"))?;
            app.manage(AppState {
                kube_client,
                http_client,
                watched_ns: std::sync::Arc::new(tokio::sync::RwLock::new(String::new())),
                log_stream: std::sync::Arc::new(Mutex::new(None)),
            });
            let watcher_client = app.state::<AppState>().kube_client.clone();
            let watcher_handle = app.handle().clone();
            let watcher_ns = app.state::<AppState>().watched_ns.clone();
            tauri::async_runtime::spawn(watcher::run_watchers(watcher_client, watcher_handle, watcher_ns));

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
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        rect,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            if window.is_visible().unwrap_or(false) {
                                let _ = window.hide();
                            } else {
                                let _ = window.show();
                                if let Ok(scale) = window.scale_factor() {
                                    let tray_pos = rect.position.to_physical::<f64>(scale);
                                    let tray_size = rect.size.to_physical::<f64>(scale);
                                    if let Ok(win_size) = window.outer_size() {
                                        let x = tray_pos.x + (tray_size.width / 2.0) - (win_size.width as f64 / 2.0);
                                        let y = tray_pos.y;
                                        let _ = window.set_position(PhysicalPosition::new(x, y));
                                    }
                                }
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
            list_pods,
            stream_logs,
            pause_log_stream,
            resume_log_stream,
            stop_log_stream,
            fetch_one_service,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
