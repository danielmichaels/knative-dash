use serde::{Serialize, Serializer};

#[derive(thiserror::Error, Debug)]
pub enum AppError {
    #[error("kubernetes error: {0}")]
    Kube(String),

    #[error("http error: {0}")]
    Http(String),

    #[error("failed to open URL: {0}")]
    Open(String),
}

impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
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
    fn error_serializes_as_display_string() {
        let e = AppError::Kube("test".into());
        let json = serde_json::to_string(&e).unwrap();
        assert_eq!(json, r#""kubernetes error: test""#);
    }
}
