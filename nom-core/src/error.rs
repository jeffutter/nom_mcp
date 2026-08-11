//! Unified error taxonomy for all four surfaces (local-CLI, remote-CLI, HTTP, MCP).
//!
//! `ErrorData` is the single error currency returned by every surface.
//! See doc-5 §10 for the complete category/status/exit-code table.

use serde::{Deserialize, Serialize};

/// Error categories that map to HTTP status codes and CLI exit codes.
/// See doc-5 §10 for the full mapping table.
#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ErrorCategory {
    NotFound,
    Validation,
    Conflict,
    ExternalApiFailure,
    StorageFailure,
}

impl ErrorCategory {
    /// Map to HTTP status code per doc-5 §10 table.
    pub fn http_status(&self) -> http::StatusCode {
        match self {
            Self::NotFound => http::StatusCode::NOT_FOUND,
            Self::Validation => http::StatusCode::BAD_REQUEST,
            Self::Conflict => http::StatusCode::CONFLICT,
            Self::ExternalApiFailure => http::StatusCode::BAD_GATEWAY,
            Self::StorageFailure => http::StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// Map to CLI exit code per doc-5 §10 table.
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::NotFound => 3,
            Self::Validation => 4,
            Self::Conflict => 5,
            Self::ExternalApiFailure => 6,
            Self::StorageFailure => 7,
        }
    }
}

impl std::fmt::Display for ErrorCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound => write!(f, "not found"),
            Self::Validation => write!(f, "validation error"),
            Self::Conflict => write!(f, "conflict"),
            Self::ExternalApiFailure => write!(f, "external API failure"),
            Self::StorageFailure => write!(f, "storage failure"),
        }
    }
}

/// Unified error data structure used across all four surfaces.
///
/// - Validation carries `{category, field, reason}`
/// - Conflict / ExternalApiFailure / StorageFailure carry `{category, reason}`
/// - NotFound carries only `{category}`
#[derive(Debug, Clone, Serialize, Deserialize, thiserror::Error)]
pub struct ErrorData {
    pub category: ErrorCategory,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl ErrorData {
    pub fn not_found() -> Self {
        Self {
            category: ErrorCategory::NotFound,
            field: None,
            reason: None,
        }
    }

    pub fn validation(field: impl Into<String>, reason: impl Into<String>) -> Self {
        Self {
            category: ErrorCategory::Validation,
            field: Some(field.into()),
            reason: Some(reason.into()),
        }
    }

    pub fn conflict(reason: impl Into<String>) -> Self {
        Self {
            category: ErrorCategory::Conflict,
            field: None,
            reason: Some(reason.into()),
        }
    }

    pub fn external_api_failure(reason: impl Into<String>) -> Self {
        Self {
            category: ErrorCategory::ExternalApiFailure,
            field: None,
            reason: Some(reason.into()),
        }
    }

    pub fn storage_failure(reason: impl Into<String>) -> Self {
        Self {
            category: ErrorCategory::StorageFailure,
            field: None,
            reason: Some(reason.into()),
        }
    }
}

impl std::fmt::Display for ErrorData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.category)?;
        if let Some(ref reason) = self.reason {
            write!(f, ": {}", reason)?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Shared render function — turns ErrorData into stderr text + exit code.
// Used by both local-CLI and remote-CLI so output is identical.
// ---------------------------------------------------------------------------

/// Render an `ErrorData` into a human-readable stderr message and the
/// corresponding CLI exit code.
///
/// The lock-probe case (`Conflict` with reason `"local_db_locked"`) gets a
/// CLI-specific friendly message.
pub fn render_error(error: &ErrorData) -> (String, i32) {
    let exit_code = error.category.exit_code();
    let message = match &error.category {
        ErrorCategory::NotFound => {
            if let Some(reason) = &error.reason {
                format!("not found: {reason}")
            } else {
                "not found".to_string()
            }
        }
        ErrorCategory::Validation => {
            let field = error.field.as_deref().unwrap_or("unknown");
            let reason = error.reason.as_deref().unwrap_or("invalid value");
            format!("validation error on field '{field}': {reason}")
        }
        ErrorCategory::Conflict => {
            if error.reason.as_deref() == Some("local_db_locked") {
                "server is running — stop it or use the remote-CLI instead".to_string()
            } else {
                let reason = error.reason.as_deref().unwrap_or("conflict");
                format!("conflict: {reason}")
            }
        }
        ErrorCategory::ExternalApiFailure => {
            let reason = error.reason.as_deref().unwrap_or("external API failure");
            format!("external API failure: {reason}")
        }
        ErrorCategory::StorageFailure => {
            let reason = error.reason.as_deref().unwrap_or("storage failure");
            format!("storage failure: {reason}")
        }
    };
    (message, exit_code)
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- ErrorCategory mapping tests --

    #[test]
    fn test_http_status_mapping() {
        assert_eq!(ErrorCategory::NotFound.http_status(), 404);
        assert_eq!(ErrorCategory::Validation.http_status(), 400);
        assert_eq!(ErrorCategory::Conflict.http_status(), 409);
        assert_eq!(ErrorCategory::ExternalApiFailure.http_status(), 502);
        assert_eq!(ErrorCategory::StorageFailure.http_status(), 500);
    }

    #[test]
    fn test_exit_code_mapping() {
        assert_eq!(ErrorCategory::NotFound.exit_code(), 3);
        assert_eq!(ErrorCategory::Validation.exit_code(), 4);
        assert_eq!(ErrorCategory::Conflict.exit_code(), 5);
        assert_eq!(ErrorCategory::ExternalApiFailure.exit_code(), 6);
        assert_eq!(ErrorCategory::StorageFailure.exit_code(), 7);
    }

    // -- Serialization shape tests --

    #[test]
    fn test_not_found_serialization() {
        let err = ErrorData::not_found();
        let json = serde_json::to_string(&err).unwrap();
        // NotFound should only have category
        assert!(json.contains("\"category\":\"NotFound\""));
        assert!(!json.contains("\"field\""));
        assert!(!json.contains("\"reason\""));
    }

    #[test]
    fn test_validation_serialization() {
        let err = ErrorData::validation("name", "must not be empty");
        let json = serde_json::to_string(&err).unwrap();
        assert!(json.contains("\"category\":\"Validation\""));
        assert!(json.contains("\"field\":\"name\""));
        assert!(json.contains("\"reason\":\"must not be empty\""));
    }

    #[test]
    fn test_conflict_serialization() {
        let err = ErrorData::conflict("duplicate entry");
        let json = serde_json::to_string(&err).unwrap();
        assert!(json.contains("\"category\":\"Conflict\""));
        assert!(json.contains("\"reason\":\"duplicate entry\""));
        assert!(!json.contains("\"field\""));
    }

    #[test]
    fn test_external_api_failure_serialization() {
        let err = ErrorData::external_api_failure("USDA API timeout");
        let json = serde_json::to_string(&err).unwrap();
        assert!(json.contains("\"category\":\"ExternalApiFailure\""));
        assert!(json.contains("\"reason\":\"USDA API timeout\""));
    }

    #[test]
    fn test_storage_failure_serialization() {
        let err = ErrorData::storage_failure("database locked");
        let json = serde_json::to_string(&err).unwrap();
        assert!(json.contains("\"category\":\"StorageFailure\""));
        assert!(json.contains("\"reason\":\"database locked\""));
    }

    // -- Round-trip serialization --

    #[test]
    fn test_round_trip_serialization() {
        let original = ErrorData::validation("email", "invalid format");
        let json = serde_json::to_string(&original).unwrap();
        let deserialized: ErrorData = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.category, ErrorCategory::Validation);
        assert_eq!(deserialized.field, Some("email".to_string()));
        assert_eq!(deserialized.reason, Some("invalid format".to_string()));
    }

    #[test]
    fn test_deserialize_minimal_json() {
        let json = r#"{"category":"NotFound"}"#;
        let err: ErrorData = serde_json::from_str(json).unwrap();
        assert_eq!(err.category, ErrorCategory::NotFound);
        assert!(err.field.is_none());
        assert!(err.reason.is_none());
    }

    // -- render_error tests --

    #[test]
    fn test_render_not_found() {
        let (msg, code) = render_error(&ErrorData::not_found());
        assert_eq!(msg, "not found");
        assert_eq!(code, 3);
    }

    #[test]
    fn test_render_validation() {
        let (msg, code) = render_error(&ErrorData::validation("name", "too short"));
        assert_eq!(msg, "validation error on field 'name': too short");
        assert_eq!(code, 4);
    }

    #[test]
    fn test_render_lock_probe() {
        let (msg, code) = render_error(&ErrorData::conflict("local_db_locked"));
        assert_eq!(
            msg,
            "server is running — stop it or use the remote-CLI instead"
        );
        assert_eq!(code, 5);
    }

    #[test]
    fn test_render_conflict() {
        let (msg, code) = render_error(&ErrorData::conflict("duplicate entry"));
        assert_eq!(msg, "conflict: duplicate entry");
        assert_eq!(code, 5);
    }

    #[test]
    fn test_render_external_api_failure() {
        let (msg, code) = render_error(&ErrorData::external_api_failure("timeout"));
        assert_eq!(msg, "external API failure: timeout");
        assert_eq!(code, 6);
    }

    #[test]
    fn test_render_storage_failure() {
        let (msg, code) = render_error(&ErrorData::storage_failure("disk full"));
        assert_eq!(msg, "storage failure: disk full");
        assert_eq!(code, 7);
    }
}
