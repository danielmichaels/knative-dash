use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub mod knative {
    use super::*;

    #[derive(CustomResource, Serialize, Deserialize, Default, Clone, Debug, JsonSchema)]
    #[kube(
        group = "serving.knative.dev",
        version = "v1",
        kind = "Service",
        plural = "services",
        namespaced,
        status = "KnativeServiceStatus"
    )]
    pub struct KnativeServiceSpec {}

    #[derive(Serialize, Deserialize, Default, Clone, Debug, JsonSchema)]
    pub struct KnativeServiceStatus {
        pub url: Option<String>,
        pub conditions: Option<Vec<Condition>>,
        #[serde(rename = "latestReadyRevisionName")]
        pub latest_ready_revision_name: Option<String>,
        #[serde(rename = "latestCreatedRevisionName")]
        pub latest_created_revision_name: Option<String>,
    }

    #[derive(Serialize, Deserialize, Clone, Debug, JsonSchema)]
    pub struct Condition {
        #[serde(rename = "type")]
        pub condition_type: String,
        pub status: String,
        pub reason: Option<String>,
        pub message: Option<String>,
    }

    #[derive(CustomResource, Serialize, Deserialize, Default, Clone, Debug, JsonSchema)]
    #[kube(
        group = "serving.knative.dev",
        version = "v1",
        kind = "Revision",
        plural = "revisions",
        namespaced,
        status = "KnativeRevisionStatus"
    )]
    pub struct RevisionSpec {
        #[serde(default)]
        pub containers: Vec<RevisionContainer>,
    }

    #[derive(Serialize, Deserialize, Default, Clone, Debug, JsonSchema)]
    pub struct KnativeRevisionStatus {
        pub conditions: Option<Vec<Condition>>,
    }

    #[derive(Serialize, Deserialize, Default, Clone, Debug, JsonSchema)]
    pub struct RevisionContainer {
        #[serde(default)]
        pub image: String,
    }
}

pub mod traefik {
    use super::*;

    #[derive(CustomResource, Serialize, Deserialize, Default, Clone, Debug, JsonSchema)]
    #[kube(
        group = "traefik.io",
        version = "v1alpha1",
        kind = "IngressRoute",
        plural = "ingressroutes",
        namespaced
    )]
    pub struct IngressRouteSpec {
        #[serde(rename = "entryPoints", default)]
        pub entry_points: Vec<String>,
        #[serde(default)]
        pub routes: Vec<Route>,
    }

    #[derive(Serialize, Deserialize, Default, Clone, Debug, JsonSchema)]
    pub struct Route {
        #[serde(rename = "match")]
        pub rule: String,
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ConditionSummary {
    pub condition_type: String,
    pub status: String,
    pub reason: Option<String>,
    pub message: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EventSummary {
    pub reason: String,
    pub message: String,
    pub count: i32,
    pub event_type: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ServiceSummary {
    pub name: String,
    pub namespace: String,
    pub url: Option<String>,
    pub ready: bool,
    pub instance_count: u32,
    pub conditions: Vec<ConditionSummary>,
    pub latest_revision: Option<String>,
    pub image: Option<String>,
    pub events: Vec<EventSummary>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PingResult {
    pub status_code: u16,
    pub latency_ms: u64,
}

#[derive(Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum LogEvent {
    Line {
        text: String,
        is_history: bool,
    },
    StreamStarted,
    Paused,
    Resumed,
    Error {
        message: String,
    },
    BufferOverflow {
        dropped_count: usize,
    },
    StreamEnded,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PodInfo {
    pub name: String,
    pub phase: String,
}

/// Extracts the first hostname from a Traefik match rule like `Host(`foo.example.com`)`.
pub fn extract_ingress_hostname(rule: &str) -> Option<String> {
    let start = rule.find('`')? + 1;
    let end = rule[start..].find('`')? + start;
    Some(rule[start..end].to_string())
}

/// Returns true when the conditions list contains a Ready condition with status "True".
pub fn extract_ready_status(conditions: Option<&Vec<knative::Condition>>) -> bool {
    conditions
        .and_then(|conds| conds.iter().find(|c| c.condition_type == "Ready"))
        .map(|c| c.status == "True")
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ready_condition(status: &str) -> knative::Condition {
        knative::Condition {
            condition_type: "Ready".into(),
            status: status.into(),
            reason: None,
            message: None,
        }
    }

    fn other_condition() -> knative::Condition {
        knative::Condition {
            condition_type: "ConfigurationsReady".into(),
            status: "True".into(),
            reason: None,
            message: None,
        }
    }

    #[test]
    fn ingress_hostname_extracted() {
        assert_eq!(
            extract_ingress_hostname("Host(`my-service.int.example.com`)"),
            Some("my-service.int.example.com".into())
        );
    }

    #[test]
    fn ingress_hostname_complex_rule() {
        assert_eq!(
            extract_ingress_hostname("Host(`foo.example.com`) && PathPrefix(`/api`)"),
            Some("foo.example.com".into())
        );
    }

    #[test]
    fn ingress_hostname_no_backticks() {
        assert_eq!(extract_ingress_hostname("Host(\"foo.example.com\")"), None);
    }

    #[test]
    fn ready_when_condition_true() {
        assert!(extract_ready_status(Some(&vec![ready_condition("True")])));
    }

    #[test]
    fn not_ready_when_condition_false() {
        assert!(!extract_ready_status(Some(&vec![ready_condition("False")])));
    }

    #[test]
    fn not_ready_when_condition_unknown() {
        assert!(!extract_ready_status(Some(&vec![ready_condition("Unknown")])));
    }

    #[test]
    fn not_ready_when_no_conditions() {
        assert!(!extract_ready_status(None));
    }

    #[test]
    fn not_ready_when_only_other_conditions() {
        assert!(!extract_ready_status(Some(&vec![other_condition()])));
    }

    #[test]
    fn ready_found_among_multiple_conditions() {
        assert!(extract_ready_status(Some(&vec![
            other_condition(),
            ready_condition("True")
        ])));
    }

    #[test]
    fn service_summary_roundtrip() {
        let summary = ServiceSummary {
            name: "my-service".into(),
            namespace: "default".into(),
            url: Some("https://my-service.example.com".into()),
            ready: true,
            instance_count: 2,
            conditions: vec![],
            latest_revision: None,
            image: None,
            events: vec![],
        };
        let json = serde_json::to_string(&summary).unwrap();
        let back: ServiceSummary = serde_json::from_str(&json).unwrap();
        assert_eq!(back.name, "my-service");
        assert!(back.ready);
    }

    #[test]
    fn ping_result_roundtrip() {
        let result = PingResult { status_code: 200, latency_ms: 312 };
        let json = serde_json::to_string(&result).unwrap();
        let back: PingResult = serde_json::from_str(&json).unwrap();
        assert_eq!(back.status_code, 200);
        assert_eq!(back.latency_ms, 312);
    }

    #[test]
    fn log_event_line_serializes_with_kind_tag() {
        let ev = LogEvent::Line { text: "hello".into(), is_history: true };
        let json = serde_json::to_value(&ev).unwrap();
        assert_eq!(json["kind"], "line");
        assert_eq!(json["text"], "hello");
        assert_eq!(json["isHistory"], true);
    }

    #[test]
    fn log_event_stream_started_serializes() {
        let ev = LogEvent::StreamStarted;
        let json = serde_json::to_value(&ev).unwrap();
        assert_eq!(json["kind"], "streamStarted");
    }

    #[test]
    fn log_event_buffer_overflow_serializes() {
        let ev = LogEvent::BufferOverflow { dropped_count: 42 };
        let json = serde_json::to_value(&ev).unwrap();
        assert_eq!(json["kind"], "bufferOverflow");
        assert_eq!(json["droppedCount"], 42);
    }

    #[test]
    fn pod_info_serializes_camel_case() {
        let pod = PodInfo { name: "my-pod".into(), phase: "Running".into() };
        let json = serde_json::to_value(&pod).unwrap();
        assert_eq!(json["name"], "my-pod");
        assert_eq!(json["phase"], "Running");
    }
}
