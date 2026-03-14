mod error;
mod logic;
mod types;
mod commands;

use commands::{get_logs, list_namespaces, list_services, open_url, ping_service};

pub struct AppState {
    pub kube_client: kube::Client,
    pub http_client: reqwest::Client,
}

// `tauri::async_runtime::block_on` drives the async kube init before the Tauri
// event loop starts. Safe here because no Tokio runtime is active yet.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let kube_client = tauri::async_runtime::block_on(kube::Client::try_default())
        .expect("failed to build kube client — check ~/.kube/config");

    let http_client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .expect("failed to build http client");

    tauri::Builder::default()
        .manage(AppState { kube_client, http_client })
        .plugin(tauri_plugin_opener::init())
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
