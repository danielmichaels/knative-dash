use std::sync::Arc;
use tokio::sync::RwLock;
use kube::runtime::{watcher, WatchStreamExt};
use futures::StreamExt;
use tauri::{Emitter, Manager};

pub async fn run_watchers(
    client: kube::Client,
    app_handle: tauri::AppHandle,
    watched_ns: Arc<RwLock<String>>,
) {
    let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(1);

    let svc_client = client.clone();
    let svc_tx = tx.clone();
    tokio::spawn(async move {
        let api: kube::Api<crate::types::knative::Service> = kube::Api::all(svc_client);
        let stream = watcher(api, watcher::Config::default()).applied_objects();
        tokio::pin!(stream);
        while let Some(svc) = stream.next().await {
            if let Ok(svc) = svc {
                svc_tx.try_send(kube::ResourceExt::name_any(&svc)).ok();
            }
        }
    });

    let pod_client = client.clone();
    let pod_tx = tx.clone();
    tokio::spawn(async move {
        let cfg = watcher::Config::default().labels(crate::logic::KNATIVE_SERVICE_LABEL);
        let api: kube::Api<k8s_openapi::api::core::v1::Pod> = kube::Api::all(pod_client);
        let stream = watcher(api, cfg).applied_objects();
        tokio::pin!(stream);
        while let Some(pod) = stream.next().await {
            if let Ok(pod) = pod {
                if let Some(svc_name) = kube::ResourceExt::labels(&pod).get(crate::logic::KNATIVE_SERVICE_LABEL) {
                    pod_tx.try_send(svc_name.clone()).ok();
                }
            }
        }
    });

    drop(tx);

    loop {
        let Some(first) = rx.recv().await else { break };
        let mut names: std::collections::HashSet<String> = std::collections::HashSet::from([first]);
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        while let Ok(n) = rx.try_recv() {
            names.insert(n);
        }

        let visible = app_handle
            .get_webview_window("main")
            .map(|w| w.is_visible().unwrap_or(false))
            .unwrap_or(false);
        if !visible {
            continue;
        }

        let ns = watched_ns.read().await.clone();
        if ns.is_empty() {
            continue;
        }

        let futs: Vec<_> = names
            .into_iter()
            .map(|name| crate::logic::fetch_one_service(client.clone(), ns.clone(), name))
            .collect();
        let results = futures::future::join_all(futs).await;
        for summary in results.into_iter().flatten().flatten() {
            app_handle.emit("service-updated", &summary).ok();
        }
    }
}
