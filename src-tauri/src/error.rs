use serde::Serialize;

#[derive(thiserror::Error, Debug, Serialize)]
pub enum AppError {
    #[error("kubernetes error: {0}")]
    Kube(String),

    #[error("http error: {0}")]
    Http(String),

    #[error("failed to open URL: {0}")]
    Open(String),

    #[error("service has no URL")]
    NoUrl,
}

impl From<kube::Error> for AppError {
    fn from(e: kube::Error) -> Self {
        AppError::Kube(e.to_string())
    }
}

impl From<reqwest::Error> for AppError {
    fn from(e: reqwest::Error) -> Self {
        AppError::Http(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display_kube() {
        let e = AppError::Kube("connection refused".into());
        assert_eq!(e.to_string(), "kubernetes error: connection refused");
    }

    #[test]
    fn error_display_http() {
        let e = AppError::Http("timeout".into());
        assert_eq!(e.to_string(), "http error: timeout");
    }

    #[test]
    fn error_display_no_url() {
        let e = AppError::NoUrl;
        assert_eq!(e.to_string(), "service has no URL");
    }

    #[test]
    fn error_is_serialize() {
        let e = AppError::Kube("test".into());
        let _json = serde_json::to_string(&e).unwrap();
    }
}
