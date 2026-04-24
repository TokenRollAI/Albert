//! Zero-dependency JSON body validator that walks a `CanonicalSchemaNode`
//! and reports the first mismatch against a `serde_json::Value`. Used by
//! `mock_handler` when `GatewayConfig.enforce_request_bodies` is on.
//!
//! The matcher is intentionally strict-but-small: it checks the declared
//! type, required object fields, array items, enum membership, and
//! non-nullability. It does not attempt to validate `one_of` / `all_of`
//! shapes (which the canonical SchemaNode doesn't model anyway), nor
//! string `format` constraints — those are explicit non-goals so the
//! gateway stays dependency-free.
//!
//! Return shape: `Ok(())` when the body matches, `Err(ValidationError)`
//! with a `path` (dot-separated) + `message` pair on the first failure.
//! Callers render the error into the 400 response body.
//!
//! # Null vs. missing
//!
//! `null` is only accepted when the schema sets `nullable: true`. A
//! missing required field produces `required field "…" missing`, not
//! a null mismatch.

use albert_core::{SchemaNode, SchemaNodeType};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationError {
    /// Dot-path to the offending field, or `$` for the root. Makes the
    /// emitted 400 response point at the broken spot the way jsonpath
    /// consumers expect.
    pub path: String,
    pub message: String,
}

impl ValidationError {
    fn new(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            message: message.into(),
        }
    }
}

/// Entry point. Validate `body` against `schema`. Returns the first
/// mismatch (not all mismatches) — the handler needs a single error for
/// the 400 response, and short-circuiting keeps large payloads cheap.
pub fn validate(body: &Value, schema: &SchemaNode) -> Result<(), ValidationError> {
    validate_at("$", body, schema)
}

fn validate_at(path: &str, body: &Value, schema: &SchemaNode) -> Result<(), ValidationError> {
    if body.is_null() {
        if schema.nullable {
            return Ok(());
        }
        if matches!(schema.node_type, SchemaNodeType::Null) {
            return Ok(());
        }
        return Err(ValidationError::new(
            path,
            format!("expected {}, got null", schema_type_name(&schema.node_type)),
        ));
    }

    // Enum check applies to any type and is cheap, so do it first.
    if !schema.enum_values.is_empty() && !schema.enum_values.iter().any(|v| v == body) {
        return Err(ValidationError::new(
            path,
            format!("value {} is not in the declared enum", compact(body)),
        ));
    }

    match &schema.node_type {
        SchemaNodeType::Object => {
            let Some(map) = body.as_object() else {
                return Err(ValidationError::new(
                    path,
                    format!("expected object, got {}", primitive_name(body)),
                ));
            };
            for (name, child_schema) in &schema.properties {
                let child_path = if path == "$" {
                    format!("$.{name}")
                } else {
                    format!("{path}.{name}")
                };
                match map.get(name) {
                    Some(child_value) => {
                        validate_at(&child_path, child_value, child_schema)?;
                    }
                    None => {
                        if child_schema.required {
                            return Err(ValidationError::new(
                                child_path,
                                format!("required field '{name}' missing"),
                            ));
                        }
                    }
                }
            }
            Ok(())
        }
        SchemaNodeType::Array => {
            let Some(items) = body.as_array() else {
                return Err(ValidationError::new(
                    path,
                    format!("expected array, got {}", primitive_name(body)),
                ));
            };
            if let Some(item_schema) = &schema.items {
                for (i, item) in items.iter().enumerate() {
                    let child_path = format!("{path}[{i}]");
                    validate_at(&child_path, item, item_schema)?;
                }
            }
            Ok(())
        }
        SchemaNodeType::String => {
            if body.is_string() {
                Ok(())
            } else {
                Err(ValidationError::new(
                    path,
                    format!("expected string, got {}", primitive_name(body)),
                ))
            }
        }
        SchemaNodeType::Integer => {
            if body.is_i64() || body.is_u64() {
                Ok(())
            } else if body.as_f64().is_some_and(|n| n.fract() == 0.0) {
                // Integers emitted as `42.0` in some JSON producers; accept
                // when the fractional part is zero.
                Ok(())
            } else {
                Err(ValidationError::new(
                    path,
                    format!("expected integer, got {}", primitive_name(body)),
                ))
            }
        }
        SchemaNodeType::Number => {
            if body.is_number() {
                Ok(())
            } else {
                Err(ValidationError::new(
                    path,
                    format!("expected number, got {}", primitive_name(body)),
                ))
            }
        }
        SchemaNodeType::Boolean => {
            if body.is_boolean() {
                Ok(())
            } else {
                Err(ValidationError::new(
                    path,
                    format!("expected boolean, got {}", primitive_name(body)),
                ))
            }
        }
        SchemaNodeType::Null => {
            if body.is_null() {
                Ok(())
            } else {
                Err(ValidationError::new(
                    path,
                    format!("expected null, got {}", primitive_name(body)),
                ))
            }
        }
        SchemaNodeType::Unknown => Ok(()),
    }
}

fn schema_type_name(t: &SchemaNodeType) -> &'static str {
    match t {
        SchemaNodeType::Object => "object",
        SchemaNodeType::Array => "array",
        SchemaNodeType::String => "string",
        SchemaNodeType::Integer => "integer",
        SchemaNodeType::Number => "number",
        SchemaNodeType::Boolean => "boolean",
        SchemaNodeType::Null => "null",
        SchemaNodeType::Unknown => "any",
    }
}

fn primitive_name(v: &Value) -> &'static str {
    match v {
        Value::Object(_) => "object",
        Value::Array(_) => "array",
        Value::String(_) => "string",
        Value::Number(n) if n.is_i64() || n.is_u64() => "integer",
        Value::Number(_) => "number",
        Value::Bool(_) => "boolean",
        Value::Null => "null",
    }
}

fn compact(v: &Value) -> String {
    let s = v.to_string();
    if s.len() > 40 {
        format!("{}…", &s.chars().take(40).collect::<String>())
    } else {
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use albert_core::SchemaNode;
    use serde_json::json;

    #[test]
    fn flat_primitive_ok_and_fail() {
        assert!(validate(&json!("hi"), &SchemaNode::string()).is_ok());
        let err = validate(&json!(42), &SchemaNode::string()).unwrap_err();
        assert_eq!(err.path, "$");
        assert!(err.message.contains("expected string"));
    }

    #[test]
    fn integer_accepts_whole_floats() {
        let mut schema = SchemaNode::string();
        schema.node_type = SchemaNodeType::Integer;
        assert!(validate(&json!(1), &schema).is_ok());
        assert!(validate(&json!(1.0), &schema).is_ok());
        let err = validate(&json!(1.5), &schema).unwrap_err();
        assert!(err.message.contains("expected integer"));
    }

    #[test]
    fn null_only_when_nullable() {
        let mut schema = SchemaNode::string();
        assert!(validate(&json!(null), &schema).is_err());
        schema.nullable = true;
        assert!(validate(&json!(null), &schema).is_ok());
    }

    #[test]
    fn required_object_field_missing_surfaces_path() {
        let mut schema = SchemaNode::object();
        let mut name = SchemaNode::string();
        name.required = true;
        schema.properties.insert("name".to_string(), name);
        let err = validate(&json!({}), &schema).unwrap_err();
        assert_eq!(err.path, "$.name");
        assert!(err.message.contains("missing"));
    }

    #[test]
    fn nested_mismatch_points_at_leaf() {
        let mut schema = SchemaNode::object();
        let mut address = SchemaNode::object();
        let mut zip = SchemaNode::string();
        zip.required = true;
        address.properties.insert("zip".to_string(), zip);
        address.required = true;
        schema.properties.insert("address".to_string(), address);
        let err = validate(&json!({"address": {"zip": 12345}}), &schema).unwrap_err();
        assert_eq!(err.path, "$.address.zip");
        assert!(err.message.contains("expected string"));
    }

    #[test]
    fn array_items_validate_by_index() {
        let schema = SchemaNode::array({
            let mut n = SchemaNode::string();
            n.node_type = SchemaNodeType::Integer;
            n
        });
        let err = validate(&json!([1, 2, "oops", 4]), &schema).unwrap_err();
        assert_eq!(err.path, "$[2]");
        assert!(err.message.contains("expected integer"));
    }

    #[test]
    fn enum_rejects_off_list_value() {
        let mut schema = SchemaNode::string();
        schema.enum_values = vec![json!("one"), json!("two")];
        assert!(validate(&json!("one"), &schema).is_ok());
        let err = validate(&json!("three"), &schema).unwrap_err();
        assert!(err.message.contains("enum"));
    }

    #[test]
    fn unknown_schema_type_accepts_anything() {
        let mut schema = SchemaNode::string();
        schema.node_type = SchemaNodeType::Unknown;
        assert!(validate(&json!({"whatever": [1, 2]}), &schema).is_ok());
    }

    #[test]
    fn missing_optional_fields_is_ok() {
        let mut schema = SchemaNode::object();
        let optional = SchemaNode::string();
        schema.properties.insert("note".to_string(), optional);
        assert!(validate(&json!({}), &schema).is_ok());
    }
}
