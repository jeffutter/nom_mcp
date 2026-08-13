//! CLI argument parsing shared between local-CLI and remote-CLI.
//!
//! Parses `key=value` argument pairs into typed JSON objects.
//! Supports auto-typing: bare numbers become JSON numbers, "true"/"false" become booleans,
//! everything else is a string.

use std::collections::HashMap;

use super::error::ErrorData;

/// Parse key=value argument pairs into a JSON object.
/// Supports auto-typing: bare numbers become JSON numbers, "true"/"false" become booleans,
/// everything else is a string.
pub fn parse_params(args: &[String]) -> Result<serde_json::Value, ErrorData> {
    let mut map = HashMap::new();
    for arg in args {
        let pos = arg.find('=').ok_or_else(|| {
            ErrorData::validation("argument", format!("expected key=value, got: {arg}"))
        })?;
        let key = &arg[..pos];
        let value = &arg[pos + 1..];
        map.insert(key.to_string(), parse_value(value));
    }
    serde_json::to_value(map)
        .map_err(|e| ErrorData::storage_failure(format!("failed to serialize params: {e}")))
}

/// Auto-type a string value: numbers → JSON number, true/false → boolean, else string.
pub fn parse_value(s: &str) -> serde_json::Value {
    if s == "true" {
        serde_json::Value::Bool(true)
    } else if s == "false" {
        serde_json::Value::Bool(false)
    } else if let Ok(n) = s.parse::<i64>() {
        serde_json::json!(n)
    } else if let Ok(f) = s.parse::<f64>() {
        serde_json::json!(f)
    } else {
        serde_json::json!(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ErrorCategory;

    // -- parse_value tests --

    #[test]
    fn test_parse_value_numbers() {
        assert_eq!(parse_value("42"), serde_json::json!(42));
        assert_eq!(parse_value("-7"), serde_json::json!(-7));
    }

    #[test]
    fn test_parse_value_floats() {
        assert_eq!(parse_value("2.71"), serde_json::Value::from(2.71_f64));
        assert_eq!(parse_value("-0.5"), serde_json::json!(-0.5));
    }

    #[test]
    fn test_parse_value_booleans() {
        assert_eq!(parse_value("true"), serde_json::json!(true));
        assert_eq!(parse_value("false"), serde_json::json!(false));
    }

    #[test]
    fn test_parse_value_strings() {
        assert_eq!(parse_value("hello"), serde_json::json!("hello"));
        assert_eq!(parse_value("123abc"), serde_json::json!("123abc"));
    }

    // -- parse_params tests --

    #[test]
    fn test_parse_params_empty() {
        let result = parse_params(&[]).unwrap();
        assert!(result.is_object());
        assert!(result.as_object().unwrap().is_empty());
    }

    #[test]
    fn test_parse_params_mixed_types() {
        let result =
            parse_params(&["name=widget".into(), "count=5".into(), "active=true".into()]).unwrap();
        assert_eq!(result["name"], "widget");
        assert_eq!(result["count"], 5);
        assert_eq!(result["active"], true);
    }

    #[test]
    fn test_parse_params_missing_equals() {
        let result = parse_params(&["bare-flag".into()]);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.category, ErrorCategory::Validation);
    }

    // AC#4: prove parse_params produces typed output matching what local-CLI needs
    #[test]
    fn test_parse_params_produces_query_for_search_food() {
        let result = parse_params(&["query=almonds".into()]).unwrap();
        assert_eq!(result["query"], "almonds");
        assert!(!result.as_object().unwrap().is_empty());
    }

    #[test]
    fn test_parse_params_empty_vs_with_args() {
        let empty = parse_params(&[]).unwrap();
        let with_query = parse_params(&["query=almonds".into()]).unwrap();
        assert!(empty.as_object().unwrap().is_empty());
        assert!(!with_query.as_object().unwrap().is_empty());
    }
}
