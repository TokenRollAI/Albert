//! Translate the live `CanonicalApiCollection`s into an OpenAPI 3.0
//! document suitable for consumption by swagger-ui, OpenAPI codegen,
//! Postman, or any other spec-aware tool. Served at
//! `/__albert/openapi.json` by the `openapi_handler`.
//!
//! The translation is lossy in two places we care about:
//!
//! - **Parameters without a declared schema** are rendered as
//!   `{"type": "string"}` so consuming tools don't trip over an empty
//!   shape. We only touch the wire format; the canonical in-memory
//!   representation is left alone.
//! - **`one_of` / `any_of` / `all_of`** aren't modeled in the canonical
//!   SchemaNode shape, so we emit plain object/array/primitive shapes.
//!   The success example is still surfaced via `components.examples`
//!   so downstream tools can pin expected bodies.

use albert_core::{
    CanonicalApiCollection, CanonicalEndpoint, CanonicalParameter, CanonicalRequestBody,
    HttpMethod, MockExampleKind, ParameterLocation, SchemaNode, SchemaNodeType,
};
use serde_json::{Map, Value, json};

/// Translate a live set of collections into an OpenAPI 3.0 document.
/// The `base_url` is used for the optional `servers` entry so spec
/// consumers can point their generated clients at the running mock.
pub fn to_openapi_document(
    collections: &[CanonicalApiCollection],
    base_url: Option<&str>,
) -> Value {
    let mut paths: Map<String, Value> = Map::new();
    let mut tags: Vec<Value> = Vec::new();
    let mut tag_seen = std::collections::BTreeSet::new();

    for collection in collections {
        if !tag_seen.contains(&collection.name) {
            tag_seen.insert(collection.name.clone());
            tags.push(json!({ "name": collection.name }));
        }
        for endpoint in &collection.endpoints {
            let path_item = paths
                .entry(endpoint.path.clone())
                .or_insert_with(|| json!({}));
            let path_object = path_item.as_object_mut().expect("path item");
            let operation_key = method_to_openapi_key(&endpoint.method);
            path_object.insert(
                operation_key.to_string(),
                operation_object(collection, endpoint),
            );
        }
    }

    let info = json!({
        "title": collections
            .first()
            .map(|c| c.name.clone())
            .unwrap_or_else(|| "Albert mock gateway".to_string()),
        "version": "1.0.0",
        "description": collections
            .iter()
            .filter_map(|c| c.description.as_deref())
            .collect::<Vec<_>>()
            .join("\n\n"),
    });

    let mut doc = json!({
        "openapi": "3.0.3",
        "info": info,
        "paths": paths,
        "tags": tags,
    });

    if let Some(url) = base_url {
        doc["servers"] = json!([{ "url": url }]);
    }

    doc
}

fn method_to_openapi_key(method: &HttpMethod) -> &'static str {
    match method {
        HttpMethod::Get => "get",
        HttpMethod::Post => "post",
        HttpMethod::Put => "put",
        HttpMethod::Patch => "patch",
        HttpMethod::Delete => "delete",
        HttpMethod::Options => "options",
        HttpMethod::Head => "head",
    }
}

fn operation_object(collection: &CanonicalApiCollection, endpoint: &CanonicalEndpoint) -> Value {
    let mut op = Map::new();
    if let Some(id) = &endpoint.operation_id {
        op.insert("operationId".to_string(), Value::String(id.clone()));
    }
    if let Some(summary) = &endpoint.summary {
        op.insert("summary".to_string(), Value::String(summary.clone()));
    }
    if let Some(description) = &endpoint.description {
        op.insert(
            "description".to_string(),
            Value::String(description.clone()),
        );
    }
    let mut tags: Vec<Value> = endpoint
        .tags
        .iter()
        .map(|t| Value::String(t.clone()))
        .collect();
    if !tags.iter().any(|t| t.as_str() == Some(&collection.name)) {
        tags.push(Value::String(collection.name.clone()));
    }
    op.insert("tags".to_string(), Value::Array(tags));
    if !endpoint.parameters.is_empty() {
        op.insert(
            "parameters".to_string(),
            Value::Array(
                endpoint
                    .parameters
                    .iter()
                    .map(parameter_object)
                    .collect::<Vec<_>>(),
            ),
        );
    }
    if let Some(body) = &endpoint.request_body {
        op.insert("requestBody".to_string(), request_body_object(body));
    }
    op.insert("responses".to_string(), responses_object(endpoint));
    Value::Object(op)
}

fn parameter_object(parameter: &CanonicalParameter) -> Value {
    let mut p = json!({
        "name": parameter.name,
        "in": parameter_location_key(&parameter.location),
        "required": parameter.required,
        "schema": schema_to_json(&parameter.schema),
    });
    if let Some(description) = &parameter.description {
        p["description"] = Value::String(description.clone());
    }
    p
}

fn parameter_location_key(location: &ParameterLocation) -> &'static str {
    match location {
        ParameterLocation::Path => "path",
        ParameterLocation::Query => "query",
        ParameterLocation::Header => "header",
        ParameterLocation::Cookie => "cookie",
    }
}

fn request_body_object(body: &CanonicalRequestBody) -> Value {
    json!({
        "required": body.required,
        "content": {
            body.content_type.clone(): {
                "schema": schema_to_json(&body.schema),
            }
        }
    })
}

fn responses_object(endpoint: &CanonicalEndpoint) -> Value {
    let mut out: Map<String, Value> = Map::new();
    for response in &endpoint.responses {
        let content_type = &response.content_type;
        let mut media_type = json!({});
        if let Some(schema) = &response.schema {
            media_type["schema"] = schema_to_json(schema);
        }
        // Pin the example when present so downstream tools see the mock payload.
        if let Some(example) = endpoint
            .examples
            .iter()
            .find(|e| matches!(e.kind, MockExampleKind::Success))
            && response.status_code.starts_with('2')
        {
            media_type["example"] = example.payload.clone();
        }
        if let Some(example) = endpoint
            .examples
            .iter()
            .find(|e| matches!(e.kind, MockExampleKind::Error))
            && !response.status_code.starts_with('2')
        {
            media_type["example"] = example.payload.clone();
        }
        let body = Value::Object({
            let mut m = Map::new();
            m.insert(
                "description".to_string(),
                Value::String(
                    response
                        .description
                        .clone()
                        .unwrap_or_else(|| response.status_code.clone()),
                ),
            );
            m.insert(
                "content".to_string(),
                Value::Object({
                    let mut inner = Map::new();
                    inner.insert(content_type.clone(), media_type);
                    inner
                }),
            );
            m
        });
        out.insert(response.status_code.clone(), body);
    }
    if out.is_empty() {
        // Always emit at least one response — spec consumers treat an
        // empty `responses` map as invalid.
        out.insert(
            "200".to_string(),
            json!({ "description": "OK (no response schema declared)" }),
        );
    }
    Value::Object(out)
}

fn schema_to_json(node: &SchemaNode) -> Value {
    let mut schema = Map::new();
    match node.node_type {
        SchemaNodeType::Object => {
            schema.insert("type".to_string(), Value::String("object".to_string()));
            if !node.properties.is_empty() {
                let mut props = Map::new();
                let mut required: Vec<Value> = Vec::new();
                for (name, child) in &node.properties {
                    props.insert(name.clone(), schema_to_json(child));
                    if child.required {
                        required.push(Value::String(name.clone()));
                    }
                }
                schema.insert("properties".to_string(), Value::Object(props));
                if !required.is_empty() {
                    schema.insert("required".to_string(), Value::Array(required));
                }
            }
        }
        SchemaNodeType::Array => {
            schema.insert("type".to_string(), Value::String("array".to_string()));
            if let Some(items) = &node.items {
                schema.insert("items".to_string(), schema_to_json(items));
            } else {
                schema.insert("items".to_string(), json!({ "type": "string" }));
            }
        }
        SchemaNodeType::String => {
            schema.insert("type".to_string(), Value::String("string".to_string()));
        }
        SchemaNodeType::Integer => {
            schema.insert("type".to_string(), Value::String("integer".to_string()));
        }
        SchemaNodeType::Number => {
            schema.insert("type".to_string(), Value::String("number".to_string()));
        }
        SchemaNodeType::Boolean => {
            schema.insert("type".to_string(), Value::String("boolean".to_string()));
        }
        SchemaNodeType::Null => {
            schema.insert("type".to_string(), Value::String("null".to_string()));
        }
        SchemaNodeType::Unknown => {
            // Unknown types stay untyped — downstream tools default to "any".
        }
    }
    if node.nullable {
        schema.insert("nullable".to_string(), Value::Bool(true));
    }
    if let Some(description) = &node.description {
        schema.insert(
            "description".to_string(),
            Value::String(description.clone()),
        );
    }
    if !node.enum_values.is_empty() {
        schema.insert("enum".to_string(), Value::Array(node.enum_values.clone()));
    }
    if let Some(example) = &node.example {
        schema.insert("example".to_string(), example.clone());
    }
    Value::Object(schema)
}

#[cfg(test)]
mod tests {
    use super::*;
    use albert_core::{
        CanonicalApiCollection, CanonicalEndpoint, CanonicalParameter, CanonicalRequestBody,
        CanonicalResponse, HttpMethod, InputSourceKind, MockExample, MockExampleKind,
        ParameterLocation, SchemaNode, SchemaNodeType,
    };
    use serde_json::json;

    fn sample_collection() -> CanonicalApiCollection {
        CanonicalApiCollection {
            id: "orders".to_string(),
            name: "Orders API".to_string(),
            source: InputSourceKind::OpenApi,
            description: Some("Demo orders API".to_string()),
            endpoints: vec![CanonicalEndpoint {
                operation_id: Some("getOrder".to_string()),
                method: HttpMethod::Get,
                path: "/orders/{id}".to_string(),
                summary: Some("Fetch one order".to_string()),
                description: None,
                tags: vec!["orders".to_string()],
                parameters: vec![CanonicalParameter {
                    name: "id".to_string(),
                    location: ParameterLocation::Path,
                    description: Some("Order id".to_string()),
                    required: true,
                    schema: SchemaNode::string(),
                }],
                request_body: None,
                responses: vec![CanonicalResponse {
                    status_code: "200".to_string(),
                    description: Some("OK".to_string()),
                    content_type: "application/json".to_string(),
                    schema: Some(SchemaNode::object()),
                }],
                examples: vec![MockExample {
                    kind: MockExampleKind::Success,
                    title: "Success".to_string(),
                    payload: json!({"id": "o-1", "status": "paid"}),
                    note: None,
                }],
                auth: None,
            }],
        }
    }

    #[test]
    fn emits_openapi_3_0_shape() {
        let doc = to_openapi_document(&[sample_collection()], Some("http://127.0.0.1:4317"));
        assert_eq!(doc["openapi"], "3.0.3");
        assert_eq!(doc["info"]["title"], "Orders API");
        assert_eq!(doc["servers"][0]["url"], "http://127.0.0.1:4317");
        let op = &doc["paths"]["/orders/{id}"]["get"];
        assert_eq!(op["operationId"], "getOrder");
        assert_eq!(op["parameters"][0]["name"], "id");
        assert_eq!(op["parameters"][0]["in"], "path");
        assert_eq!(op["responses"]["200"]["description"], "OK");
        assert_eq!(
            op["responses"]["200"]["content"]["application/json"]["example"]["id"],
            "o-1"
        );
    }

    #[test]
    fn omits_servers_when_base_url_absent() {
        let doc = to_openapi_document(&[sample_collection()], None);
        assert!(doc.get("servers").is_none());
    }

    #[test]
    fn request_body_round_trips_content_type_and_required() {
        let mut c = sample_collection();
        c.endpoints[0].method = HttpMethod::Post;
        c.endpoints[0].path = "/orders".to_string();
        c.endpoints[0].request_body = Some(CanonicalRequestBody {
            content_type: "application/json".to_string(),
            required: true,
            schema: SchemaNode::object(),
        });
        let doc = to_openapi_document(&[c], None);
        let body = &doc["paths"]["/orders"]["post"]["requestBody"];
        assert_eq!(body["required"], true);
        assert!(body["content"]["application/json"]["schema"].is_object());
    }

    #[test]
    fn empty_responses_get_a_synthetic_200() {
        let mut c = sample_collection();
        c.endpoints[0].responses.clear();
        let doc = to_openapi_document(&[c], None);
        let responses = &doc["paths"]["/orders/{id}"]["get"]["responses"];
        assert!(responses["200"]["description"].as_str().is_some());
    }

    #[test]
    fn nested_object_properties_translate_to_schemas() {
        let mut inner = SchemaNode::object();
        inner.properties.insert("amount".to_string(), {
            let mut n = SchemaNode {
                node_type: SchemaNodeType::Integer,
                required: true,
                ..SchemaNode::string()
            };
            n.node_type = SchemaNodeType::Integer;
            n
        });
        let mut c = sample_collection();
        c.endpoints[0].responses[0].schema = Some(inner);
        let doc = to_openapi_document(&[c], None);
        let schema = &doc["paths"]["/orders/{id}"]["get"]["responses"]["200"]["content"]["application/json"]
            ["schema"];
        assert_eq!(schema["type"], "object");
        assert_eq!(schema["properties"]["amount"]["type"], "integer");
        assert_eq!(schema["required"][0], "amount");
    }
}
