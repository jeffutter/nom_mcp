//! CLI argument parsing shared between local-CLI and remote-CLI.
//!
//! Parses `key=value` argument pairs into typed JSON objects.
//! Values that are valid JSON (numbers, booleans, null, arrays, objects) are sent as
//! that JSON; anything else stays a plain string.

use std::collections::HashMap;

use super::error::ErrorData;

/// Parse key=value argument pairs into a JSON object.
/// Values that are valid JSON (numbers, booleans, null, arrays, objects) are sent as
/// that JSON; anything else stays a plain string.
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

/// Auto-type a string value: values that are valid JSON (numbers, booleans,
/// null, arrays, objects) become that JSON value; anything else stays a plain
/// string. Shared by the local-CLI router and the remote-CLI so both surfaces
/// type identical inputs alike.
pub fn parse_value(s: &str) -> serde_json::Value {
    // Try parsing as JSON first (handles numbers, booleans, null, arrays, objects)
    if let Ok(val) = serde_json::from_str(s) {
        return val;
    }
    serde_json::Value::String(s.to_string())
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

    #[test]
    fn test_parse_value_arrays() {
        assert_eq!(parse_value("[\"a\",\"b\"]"), serde_json::json!(["a", "b"]));
    }

    #[test]
    fn test_parse_value_objects() {
        assert_eq!(
            parse_value("{\"k\":\"v\"}"),
            serde_json::json!({ "k": "v" })
        );
    }

    #[test]
    fn test_parse_value_null() {
        assert_eq!(parse_value("null"), serde_json::Value::Null);
    }

    /// Pins the exact log_meal portions shape from the ticket: nested array of
    /// objects must round-trip with correct field types (i64 / f64 / string).
    #[test]
    fn test_parse_value_log_meal_portions_shape() {
        let v = parse_value("[{\"food_id\":1,\"quantity\":250,\"quantity_mode\":\"grams\"}]");
        let arr = v.as_array().expect("portions must parse as an array");
        assert_eq!(arr.len(), 1);
        let portion = &arr[0];
        assert_eq!(portion["food_id"], 1);
        assert_eq!(portion["quantity"], 250);
        assert_eq!(portion["quantity_mode"], "grams");
    }

    /// Double-quoted input loses its quotes (same as the local CLI today).
    #[test]
    fn test_parse_value_quoted_string_strips_quotes() {
        assert_eq!(parse_value("\"quoted\""), serde_json::json!("quoted"));
    }

    /// The exact remote-CLI failure mode from the ticket description: a raw
    /// bracketed JSON arg must reach the params map as a real array, not a string.
    #[test]
    fn test_parse_params_nested_json_round_trip() {
        let result = parse_params(&["portions=[{\"food_id\":1}]".into()]).unwrap();
        let portions = result["portions"]
            .as_array()
            .expect("portions must be a JSON array, not a string");
        assert_eq!(portions[0]["food_id"], 1);
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
