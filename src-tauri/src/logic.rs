use k8s_openapi::api::core::v1::{Event, Pod};
use kube::api::{ListParams, LogParams};
use kube::{Api, Client, ResourceExt};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use futures::{AsyncBufReadExt, StreamExt};
use tauri::ipc::Channel;

use crate::error::AppError;
use crate::types::{
    extract_ingress_hostname, extract_ready_status, knative, traefik, ConditionSummary,
    EventSummary, LogEvent, PingResult, PodInfo, ServiceSummary,
};

pub(crate) const KNATIVE_SERVICE_LABEL: &str = "serving.knative.dev/service";
const POD_PHASE_RUNNING: &str = "Running";

static STREAM_GENERATION: AtomicU64 = AtomicU64::new(0);

pub struct LogStreamHandle {
    pub abort_handle: tokio::task::AbortHandle,
    pub paused: Arc<AtomicBool>,
    pub channel: Channel<LogEvent>,
    generation: u64,
}

fn summarize_conditions(
    conditions: Option<&Vec<knative::Condition>>,
) -> Vec<ConditionSummary> {
    conditions
        .map(|conds| {
            conds
                .iter()
                .map(|c| ConditionSummary {
                    condition_type: c.condition_type.clone(),
                    status: c.status.clone(),
                    reason: c.reason.clone(),
                    message: c.message.clone(),
                })
                .collect()
        })
        .unwrap_or_default()
}

fn is_pod_running(pod: &Pod) -> bool {
    pod.status
        .as_ref()
        .and_then(|s| s.phase.as_deref())
        == Some(POD_PHASE_RUNNING)
}

fn truncate_to_last_n(events: &mut Vec<EventSummary>, n: usize) {
    let keep = events.len().saturating_sub(n);
    events.drain(..keep);
}

fn ingress_route_to_url(ir: &traefik::IngressRoute) -> Option<String> {
    let rule = ir.spec.routes.first().map(|r| r.rule.as_str())?;
    let host = extract_ingress_hostname(rule)?;
    let scheme = if ir.spec.entry_points.iter().any(|e| e == "websecure") {
        "https"
    } else {
        "http"
    };
    Some(format!("{scheme}://{host}"))
}

fn to_event_summary(ev: Event) -> EventSummary {
    EventSummary {
        reason: ev.reason.unwrap_or_default(),
        message: ev.message.unwrap_or_default(),
        count: ev.count.unwrap_or(1),
        event_type: ev.type_.unwrap_or_else(|| "Normal".to_string()),
    }
}

/// Returns namespace names that contain at least one Knative service.
pub async fn fetch_namespaces_with_services(client: Client) -> Result<Vec<String>, AppError> {
    let api: Api<knative::Service> = Api::all(client);
    let services = api.list(&ListParams::default()).await?;

    let mut namespaces: HashSet<String> = HashSet::new();
    for svc in &services.items {
        if let Some(ns) = svc.namespace() {
            namespaces.insert(ns);
        }
    }

    let mut result: Vec<String> = namespaces.into_iter().collect();
    result.sort();
    Ok(result)
}

/// Returns all Knative services in the given namespace as `ServiceSummary` values.
/// External URLs are resolved from matching Traefik IngressRoutes when available.
/// Also enriches each service with conditions, latest revision, image tag, and recent events.
pub async fn fetch_services(
    client: Client,
    namespace: String,
) -> Result<Vec<ServiceSummary>, AppError> {
    let svc_api: Api<knative::Service> = Api::namespaced(client.clone(), &namespace);
    let ir_api: Api<traefik::IngressRoute> = Api::namespaced(client.clone(), &namespace);
    let rev_api: Api<knative::Revision> = Api::namespaced(client.clone(), &namespace);
    let event_api: Api<Event> = Api::namespaced(client.clone(), &namespace);
    let pod_api: Api<Pod> = Api::namespaced(client, &namespace);

    let lp = &ListParams::default();
    let pod_lp = &ListParams::default().labels(KNATIVE_SERVICE_LABEL);
    let (services, ingress_routes, revisions, events, pods) = tokio::join!(
        svc_api.list(lp),
        ir_api.list(lp),
        rev_api.list(lp),
        event_api.list(lp),
        pod_api.list(pod_lp),
    );
    let services = services?;

    // Build name → external URL map from IngressRoutes. Silently ignore errors
    // (RBAC may not permit listing IngressRoutes).
    let external_urls: HashMap<String, String> = ingress_routes
        .map(|list| list.items)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|ir| {
            let name = ir.name_any();
            Some((name, ingress_route_to_url(&ir)?))
        })
        .collect();

    // Build revision name → image map. Silently ignore errors (RBAC).
    let revision_images: HashMap<String, String> = revisions
        .map(|list| list.items)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|rev| {
            let name = rev.name_any();
            let image = rev.spec.containers.into_iter().next().map(|c| c.image)?;
            if image.is_empty() { return None; }
            Some((name, image))
        })
        .collect();

    // Count running pods per service. One namespace-wide list, grouped by
    // the `serving.knative.dev/service` label. Silently ignore RBAC errors.
    let mut instance_counts: HashMap<String, u32> = HashMap::new();
    for pod in pods.map(|list| list.items).unwrap_or_default() {
        if !is_pod_running(&pod) {
            continue;
        }
        if let Some(svc_name) = pod
            .labels()
            .get(KNATIVE_SERVICE_LABEL)
            .map(|s| s.to_owned())
        {
            *instance_counts.entry(svc_name).or_default() += 1;
        }
    }

    // Build service name → recent events map. Filter to Knative Service objects only,
    // keep the 5 most recent by lastTimestamp (or creationTimestamp as fallback).
    let mut service_events: HashMap<String, Vec<EventSummary>> = HashMap::new();
    for ev in events.map(|list| list.items).unwrap_or_default() {
        let obj_ref = &ev.involved_object;
        let is_knative_service = obj_ref.kind.as_deref() == Some("Service")
            && obj_ref
                .api_version
                .as_deref()
                .map(|v| v.starts_with("serving.knative.dev"))
                .unwrap_or(false);
        if !is_knative_service {
            continue;
        }
        let svc_name = match obj_ref.name.as_deref() {
            Some(n) => n.to_string(),
            None => continue,
        };
        service_events.entry(svc_name).or_default().push(to_event_summary(ev));
    }
    // Keep only the last 5 events per service (list order from API is oldest-first).
    for events in service_events.values_mut() {
        truncate_to_last_n(events, 5);
    }

    let summaries = services
        .items
        .into_iter()
        .map(|svc| {
            let name = svc.name_any();
            let status = svc.status.as_ref();
            let ready = extract_ready_status(status.and_then(|s| s.conditions.as_ref()));
            let url = external_urls
                .get(&name)
                .cloned()
                .or_else(|| status.and_then(|s| s.url.clone()));

            let conditions =
                summarize_conditions(status.and_then(|s| s.conditions.as_ref()));

            let latest_revision =
                status.and_then(|s| s.latest_ready_revision_name.clone());

            let image = latest_revision
                .as_deref()
                .and_then(|rev| revision_images.get(rev).cloned());

            let instance_count = instance_counts.get(&name).copied().unwrap_or(0);

            let events = service_events.remove(&name).unwrap_or_default();

            ServiceSummary {
                name,
                namespace: namespace.clone(),
                url,
                ready,
                instance_count,
                conditions,
                latest_revision,
                image,
                events,
            }
        })
        .collect();

    Ok(summaries)
}

/// Returns a single Knative service by name as a `ServiceSummary`, or `None` if not found.
/// External URL is resolved from a matching Traefik IngressRoute when available.
/// Enriches the summary with conditions, latest revision, image tag, running pod count,
/// and the 5 most recent events for this service.
pub async fn fetch_one_service(
    client: Client,
    namespace: String,
    name: String,
) -> Result<Option<ServiceSummary>, AppError> {
    let svc_api: Api<knative::Service> = Api::namespaced(client.clone(), &namespace);
    let ir_api: Api<traefik::IngressRoute> = Api::namespaced(client.clone(), &namespace);
    let rev_api: Api<knative::Revision> = Api::namespaced(client.clone(), &namespace);
    let event_api: Api<Event> = Api::namespaced(client.clone(), &namespace);
    let pod_api: Api<Pod> = Api::namespaced(client, &namespace);

    let svc = match svc_api.get(&name).await {
        Ok(s) => s,
        Err(kube::Error::Api(ref e)) if e.code == 404 => return Ok(None),
        Err(e) => return Err(AppError::Kube(e.to_string())),
    };

    let svc_lp = ListParams::default().labels(&format!("{KNATIVE_SERVICE_LABEL}={name}"));
    let event_lp = ListParams::default()
        .fields(&format!("involvedObject.name={name},involvedObject.kind=Service"));
    let (pods, revisions, ir, events) = tokio::join!(
        pod_api.list(&svc_lp),
        rev_api.list(&svc_lp),
        ir_api.get(&name),
        event_api.list(&event_lp),
    );

    let url = ir
        .ok()
        .as_ref()
        .and_then(ingress_route_to_url)
        .or_else(|| svc.status.as_ref().and_then(|s| s.url.clone()));

    let status = svc.status.as_ref();
    let ready = extract_ready_status(status.and_then(|s| s.conditions.as_ref()));
    let conditions = summarize_conditions(status.and_then(|s| s.conditions.as_ref()));
    let latest_revision = status.and_then(|s| s.latest_ready_revision_name.clone());

    let image = latest_revision.as_deref().and_then(|rev_name| {
        revisions.ok()?.items.into_iter()
            .find(|r| r.name_any() == rev_name)
            .and_then(|r| r.spec.containers.into_iter().next())
            .map(|c| c.image)
            .filter(|i| !i.is_empty())
    });

    let instance_count = pods
        .map(|list| list.items)
        .unwrap_or_default()
        .iter()
        .filter(|pod| is_pod_running(pod))
        .count() as u32;

    let mut svc_events: Vec<EventSummary> = events
        .map(|list| list.items)
        .unwrap_or_default()
        .into_iter()
        .map(to_event_summary)
        .collect();

    truncate_to_last_n(&mut svc_events, 5);

    Ok(Some(ServiceSummary {
        name: svc.name_any(),
        namespace,
        url,
        ready,
        instance_count,
        conditions,
        latest_revision,
        image,
        events: svc_events,
    }))
}

pub async fn list_pods(
    client: Client,
    namespace: String,
    service_name: String,
) -> Result<Vec<PodInfo>, AppError> {
    let pod_api: Api<Pod> = Api::namespaced(client, &namespace);
    let selector = format!("{KNATIVE_SERVICE_LABEL}={service_name}");
    let pods = pod_api
        .list(&ListParams::default().labels(&selector))
        .await?;

    let result = pods
        .items
        .iter()
        .map(|pod| {
            let name = pod.name_any();
            let phase = pod
                .status
                .as_ref()
                .and_then(|s| s.phase.clone())
                .unwrap_or_else(|| "Unknown".to_string());
            PodInfo { name, phase }
        })
        .collect();

    Ok(result)
}

pub fn start_log_stream(
    client: Client,
    namespace: String,
    pod_name: String,
    tail_lines: i64,
    channel: Channel<LogEvent>,
    log_stream: Arc<Mutex<Option<LogStreamHandle>>>,
) -> Result<(), AppError> {
    let generation = STREAM_GENERATION.fetch_add(1, Ordering::Relaxed) + 1;

    {
        let mut guard = log_stream.lock().expect("log_stream mutex poisoned");
        if let Some(handle) = guard.take() {
            handle.abort_handle.abort();
        }
    }

    let pod_api: Api<Pod> = Api::namespaced(client, &namespace);
    let paused = Arc::new(AtomicBool::new(false));
    let paused_clone = paused.clone();
    let channel_clone = channel.clone();
    let log_stream_clone = log_stream.clone();

    let task = tokio::spawn(async move {
        let log_params = LogParams {
            container: Some("user-container".to_string()),
            follow: true,
            tail_lines: Some(tail_lines),
            ..Default::default()
        };

        let stream = match pod_api.log_stream(&pod_name, &log_params).await {
            Ok(s) => s,
            Err(e) => {
                let _ = channel_clone.send(LogEvent::Error {
                    message: e.to_string(),
                });
                clear_if_current(&log_stream_clone, generation);
                return;
            }
        };

        if channel_clone.send(LogEvent::StreamStarted).is_err() {
            clear_if_current(&log_stream_clone, generation);
            return;
        }

        let mut lines = stream.lines();
        let mut is_history = true;
        let mut buffer: VecDeque<String> = VecDeque::new();
        let mut dropped_count: usize = 0;

        loop {
            let line_result: Option<Result<String, std::io::Error>> = if is_history {
                match tokio::time::timeout(
                    std::time::Duration::from_millis(500),
                    lines.next(),
                )
                .await
                {
                    Ok(result) => result,
                    Err(_) => {
                        is_history = false;
                        continue;
                    }
                }
            } else {
                lines.next().await
            };

            match line_result {
                Some(Ok(line)) => {
                    if paused_clone.load(Ordering::Relaxed) {
                        buffer.push_back(line);
                        if buffer.len() > 10_000 {
                            buffer.pop_front();
                            dropped_count += 1;
                        }
                        continue;
                    }

                    if dropped_count > 0 {
                        if channel_clone
                            .send(LogEvent::BufferOverflow { dropped_count })
                            .is_err()
                        {
                            break;
                        }
                        dropped_count = 0;
                    }

                    while let Some(buffered) = buffer.pop_front() {
                        if channel_clone
                            .send(LogEvent::Line {
                                text: buffered,
                                is_history: false,
                            })
                            .is_err()
                        {
                            return;
                        }
                    }

                    if channel_clone
                        .send(LogEvent::Line {
                            text: line,
                            is_history,
                        })
                        .is_err()
                    {
                        break;
                    }
                }
                None => {
                    let _ = channel_clone.send(LogEvent::StreamEnded);
                    break;
                }
                Some(Err(e)) => {
                    let _ = channel_clone.send(LogEvent::Error {
                        message: e.to_string(),
                    });
                    break;
                }
            }
        }

        clear_if_current(&log_stream_clone, generation);
    });

    let mut guard = log_stream.lock().expect("log_stream mutex poisoned");
    *guard = Some(LogStreamHandle {
        abort_handle: task.abort_handle(),
        paused,
        channel,
        generation,
    });

    Ok(())
}

fn clear_if_current(log_stream: &Mutex<Option<LogStreamHandle>>, generation: u64) {
    let mut guard = log_stream.lock().expect("log_stream mutex poisoned");
    if guard.as_ref().is_some_and(|h| h.generation == generation) {
        *guard = None;
    }
}

/// Sends an HTTP GET to `url` and returns status code and latency.
pub async fn execute_ping(
    http_client: &reqwest::Client,
    url: String,
) -> Result<PingResult, AppError> {
    let start = std::time::Instant::now();
    let response = http_client.get(&url).send().await?;
    let latency_ms = start.elapsed().as_millis() as u64;
    Ok(PingResult {
        status_code: response.status().as_u16(),
        latency_ms,
    })
}
