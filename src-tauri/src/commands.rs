use tauri::ipc::Channel;
use tauri_plugin_opener::OpenerExt;

use crate::error::AppError;
use crate::logic::{
    execute_ping, fetch_namespaces_with_services, fetch_services, list_pods as logic_list_pods,
    start_log_stream,
};
use crate::types::{LogEvent, PingResult, PodInfo, ServiceSummary};
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
pub async fn list_pods(
    namespace: String,
    service_name: String,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<PodInfo>, AppError> {
    logic_list_pods(state.kube_client.clone(), namespace, service_name).await
}

#[tauri::command]
pub async fn stream_logs(
    namespace: String,
    pod_name: String,
    tail_lines: i64,
    channel: Channel<LogEvent>,
    state: tauri::State<'_, AppState>,
) -> Result<(), AppError> {
    start_log_stream(
        state.kube_client.clone(),
        namespace,
        pod_name,
        tail_lines,
        channel,
        state.log_stream.clone(),
    )
}

#[tauri::command]
pub async fn pause_log_stream(
    state: tauri::State<'_, AppState>,
) -> Result<(), AppError> {
    set_log_paused(&state, true, LogEvent::Paused);
    Ok(())
}

#[tauri::command]
pub async fn resume_log_stream(
    state: tauri::State<'_, AppState>,
) -> Result<(), AppError> {
    set_log_paused(&state, false, LogEvent::Resumed);
    Ok(())
}

#[tauri::command]
pub async fn stop_log_stream(
    state: tauri::State<'_, AppState>,
) -> Result<(), AppError> {
    let mut guard = state.log_stream.lock().expect("log_stream mutex poisoned");
    if let Some(handle) = guard.take() {
        let _ = handle.channel.send(LogEvent::StreamEnded);
        handle.abort_handle.abort();
    }
    Ok(())
}

fn set_log_paused(state: &AppState, paused: bool, event: LogEvent) {
    let guard = state.log_stream.lock().expect("log_stream mutex poisoned");
    if let Some(handle) = guard.as_ref() {
        handle
            .paused
            .store(paused, std::sync::atomic::Ordering::Relaxed);
        let _ = handle.channel.send(event);
    }
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
