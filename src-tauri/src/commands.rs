use tauri_plugin_opener::OpenerExt;

use crate::error::AppError;
use crate::logic::{execute_ping, fetch_logs, fetch_namespaces_with_services, fetch_services};
use crate::types::{PingResult, ServiceSummary};
use crate::AppState;

#[tauri::command]
pub async fn list_namespaces(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<String>, AppError> {
    fetch_namespaces_with_services(state.kube_client.clone()).await
}

#[tauri::command]
pub async fn list_services(
    namespace: String,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<ServiceSummary>, AppError> {
    let result = fetch_services(state.kube_client.clone(), namespace.clone()).await?;
    *state.watched_ns.write().await = namespace;
    Ok(result)
}

#[tauri::command]
pub async fn ping_service(
    url: String,
    state: tauri::State<'_, AppState>,
) -> Result<PingResult, AppError> {
    execute_ping(&state.http_client, url).await
}

#[tauri::command]
pub async fn get_logs(
    namespace: String,
    service_name: String,
    state: tauri::State<'_, AppState>,
) -> Result<String, AppError> {
    fetch_logs(state.kube_client.clone(), namespace, service_name, 100).await
}

#[tauri::command]
pub async fn open_url(url: String, app: tauri::AppHandle) -> Result<(), AppError> {
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| AppError::Open(e.to_string()))
}

#[tauri::command]
pub async fn fetch_one_service(
    namespace: String,
    name: String,
    state: tauri::State<'_, AppState>,
) -> Result<Option<ServiceSummary>, AppError> {
    crate::logic::fetch_one_service(state.kube_client.clone(), namespace, name).await
}
