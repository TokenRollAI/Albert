use std::collections::{BTreeMap, BTreeSet};

use albert_core::{
    AuthRequirement, AuthScheme, CanonicalApiCollection, CanonicalEndpoint, CanonicalParameter,
    CanonicalRequestBody, CanonicalResponse, HttpMethod, InputSourceKind, ParameterLocation,
    SchemaNode, SchemaNodeType, synthesize_examples,
};
use openapiv3::{
    APIKeyLocation, AdditionalProperties, Components, MediaType, ObjectType, OpenAPI, Operation,
    Parameter, ParameterSchemaOrContent, PathItem, ReferenceOr, RequestBody, Response, Schema,
    SchemaKind, SecurityRequirement, SecurityScheme, StatusCode, Type, VariantOrUnknownOrEmpty,
};
use serde_json::Value;

use crate::{ApiParser, ParseError, ParseSource, schema_from_json_value};

#[derive(Debug, Default)]
pub struct OpenApiParser;

impl ApiParser for OpenApiParser {
    fn kind(&self) -> InputSourceKind {
        InputSourceKind::OpenApi
    }

    fn parse(&self, source: ParseSource) -> Result<CanonicalApiCollection, ParseError> {
        if source.body.trim().is_empty() {
            return Err(ParseError::InvalidSource(
                "OpenAPI content cannot be empty".to_string(),
            ));
        }

        let spec = parse_document(&source.body)?;
        let raw_spec = parse_raw_document(&source.body)?;
        let components = spec.components.as_ref();
        let collection_name = source.name.unwrap_or_else(|| spec.info.title.clone());
        let context = OpenApiParseContext {
            components,
            default_security: spec.security.as_deref(),
            raw_spec: &raw_spec,
        };

        let mut endpoints = Vec::new();
        for (path, item_ref) in spec.paths.iter() {
            let Some(path_item) = item_ref.as_item() else {
                continue;
            };
            let raw_path_item = raw_spec.get("paths").and_then(|paths| paths.get(path));

            endpoints.extend(path_item_to_endpoints(
                path,
                path_item,
                raw_path_item,
                &context,
            )?);
        }

        Ok(CanonicalApiCollection {
            id: canonical_id(&collection_name),
            name: collection_name,
            source: InputSourceKind::OpenApi,
            description: spec.info.description.clone(),
            endpoints,
        })
    }
}

fn parse_document(body: &str) -> Result<OpenAPI, ParseError> {
    serde_json::from_str::<OpenAPI>(body)
        .or_else(|_| serde_yaml::from_str::<OpenAPI>(body))
        .map_err(|error| ParseError::ParseFailed(format!("failed to deserialize OpenAPI: {error}")))
        .or_else(|_| parse_sanitized_document(body))
}

fn parse_sanitized_document(body: &str) -> Result<OpenAPI, ParseError> {
    let mut raw = serde_json::from_str::<Value>(body)
        .or_else(|_| serde_yaml::from_str::<Value>(body))
        .map_err(|error| {
            ParseError::ParseFailed(format!("failed to deserialize OpenAPI: {error}"))
        })?;
    sanitize_boolean_schemas_in_document(&mut raw);
    serde_json::from_value(raw)
        .map_err(|error| ParseError::ParseFailed(format!("failed to deserialize OpenAPI: {error}")))
}

fn sanitize_boolean_schemas_in_document(raw: &mut Value) {
    if let Some(components) = raw.get_mut("components") {
        if let Some(schemas) = components.get_mut("schemas").and_then(Value::as_object_mut) {
            for schema in schemas.values_mut() {
                sanitize_schema_position(schema);
            }
        }
        if let Some(responses) = components
            .get_mut("responses")
            .and_then(Value::as_object_mut)
        {
            for response in responses.values_mut() {
                sanitize_response_position(response);
            }
        }
        if let Some(request_bodies) = components
            .get_mut("requestBodies")
            .and_then(Value::as_object_mut)
        {
            for body in request_bodies.values_mut() {
                sanitize_request_body_position(body);
            }
        }
        if let Some(parameters) = components
            .get_mut("parameters")
            .and_then(Value::as_object_mut)
        {
            for parameter in parameters.values_mut() {
                sanitize_parameter_position(parameter);
            }
        }
    }

    if let Some(paths) = raw.get_mut("paths").and_then(Value::as_object_mut) {
        for path_item in paths.values_mut() {
            let Some(path_item) = path_item.as_object_mut() else {
                continue;
            };
            if let Some(parameters) = path_item
                .get_mut("parameters")
                .and_then(Value::as_array_mut)
            {
                for parameter in parameters {
                    sanitize_parameter_position(parameter);
                }
            }
            for method in [
                "get", "put", "post", "delete", "options", "head", "patch", "trace",
            ] {
                if let Some(operation) = path_item.get_mut(method).and_then(Value::as_object_mut) {
                    if let Some(parameters) = operation
                        .get_mut("parameters")
                        .and_then(Value::as_array_mut)
                    {
                        for parameter in parameters {
                            sanitize_parameter_position(parameter);
                        }
                    }
                    if let Some(request_body) = operation.get_mut("requestBody") {
                        sanitize_request_body_position(request_body);
                    }
                    if let Some(responses) = operation
                        .get_mut("responses")
                        .and_then(Value::as_object_mut)
                    {
                        for response in responses.values_mut() {
                            sanitize_response_position(response);
                        }
                    }
                }
            }
        }
    }
}

fn sanitize_parameter_position(value: &mut Value) {
    let value = maybe_sanitize_reference_target(value);
    if let Some(schema) = value.get_mut("schema") {
        sanitize_schema_position(schema);
    }
    if let Some(content) = value.get_mut("content").and_then(Value::as_object_mut) {
        for media in content.values_mut() {
            if let Some(schema) = media.get_mut("schema") {
                sanitize_schema_position(schema);
            }
        }
    }
}

fn sanitize_request_body_position(value: &mut Value) {
    let value = maybe_sanitize_reference_target(value);
    if let Some(content) = value.get_mut("content").and_then(Value::as_object_mut) {
        for media in content.values_mut() {
            if let Some(schema) = media.get_mut("schema") {
                sanitize_schema_position(schema);
            }
        }
    }
}

fn sanitize_response_position(value: &mut Value) {
    let value = maybe_sanitize_reference_target(value);
    if let Some(content) = value.get_mut("content").and_then(Value::as_object_mut) {
        for media in content.values_mut() {
            if let Some(schema) = media.get_mut("schema") {
                sanitize_schema_position(schema);
            }
        }
    }
}

fn maybe_sanitize_reference_target(value: &mut Value) -> &mut Value {
    if value.is_boolean() {
        *value = Value::Object(serde_json::Map::new());
    }
    value
}

fn sanitize_schema_position(value: &mut Value) {
    if value.is_boolean() {
        *value = Value::Object(serde_json::Map::new());
        return;
    }
    let Some(schema) = value.as_object_mut() else {
        return;
    };
    if let Some(properties) = schema.get_mut("properties").and_then(Value::as_object_mut) {
        for property in properties.values_mut() {
            sanitize_schema_position(property);
        }
    }
    for key in [
        "items",
        "contains",
        "additionalProperties",
        "unevaluatedProperties",
        "unevaluatedItems",
        "if",
        "then",
        "else",
    ] {
        if let Some(child) = schema.get_mut(key) {
            sanitize_schema_position(child);
        }
    }
    for key in ["prefixItems", "allOf", "anyOf", "oneOf"] {
        if let Some(items) = schema.get_mut(key).and_then(Value::as_array_mut) {
            for item in items {
                sanitize_schema_position(item);
            }
        }
    }
    if let Some(dependent_schemas) = schema
        .get_mut("dependentSchemas")
        .and_then(Value::as_object_mut)
    {
        for dependent in dependent_schemas.values_mut() {
            sanitize_schema_position(dependent);
        }
    }
}

fn parse_raw_document(body: &str) -> Result<Value, ParseError> {
    serde_json::from_str::<Value>(body)
        .or_else(|_| serde_yaml::from_str::<Value>(body))
        .map_err(|error| ParseError::ParseFailed(format!("failed to deserialize OpenAPI: {error}")))
}

struct OpenApiParseContext<'a> {
    components: Option<&'a Components>,
    default_security: Option<&'a [SecurityRequirement]>,
    raw_spec: &'a Value,
}

fn path_item_to_endpoints(
    path: &str,
    path_item: &PathItem,
    raw_path_item: Option<&Value>,
    context: &OpenApiParseContext<'_>,
) -> Result<Vec<CanonicalEndpoint>, ParseError> {
    let mut endpoints = Vec::new();

    for (method, operation) in path_item.iter() {
        let raw_operation = raw_path_item.and_then(|item| item.get(method));
        endpoints.push(operation_to_endpoint(
            path,
            method,
            path_item,
            operation,
            raw_operation,
            context,
        )?);
    }

    Ok(endpoints)
}

fn operation_to_endpoint(
    path: &str,
    method: &str,
    path_item: &PathItem,
    operation: &Operation,
    raw_operation: Option<&Value>,
    context: &OpenApiParseContext<'_>,
) -> Result<CanonicalEndpoint, ParseError> {
    let mut parameters = BTreeMap::new();

    for parameter in &path_item.parameters {
        let canonical = parameter_to_canonical(parameter, context.components)?;
        parameters.insert(parameter_key(&canonical), canonical);
    }

    for parameter in &operation.parameters {
        let canonical = parameter_to_canonical(parameter, context.components)?;
        parameters.insert(parameter_key(&canonical), canonical);
    }

    let request_body = operation
        .request_body
        .as_ref()
        .map(|body| {
            request_body_to_canonical(
                body,
                context.components,
                raw_operation.and_then(|operation| operation.get("requestBody")),
                context.raw_spec,
            )
        })
        .transpose()?;

    let responses = responses_to_canonical(
        &operation.responses.responses,
        context.components,
        raw_operation
            .and_then(|operation| operation.get("responses"))
            .map(|responses| resolve_raw_reference_if_needed(responses, context.raw_spec))
            .unwrap_or(&Value::Null),
        context.raw_spec,
    )?;

    // Operation-level security overrides the top-level default (even when
    // empty — an empty list means "explicitly no auth required").
    let effective_security = operation.security.as_deref().or(context.default_security);
    let auth = effective_security.and_then(|reqs| resolve_auth_hint(reqs, context.components));

    let mut endpoint = CanonicalEndpoint {
        operation_id: operation.operation_id.clone(),
        method: http_method_from_str(method)?,
        path: path.to_string(),
        summary: operation
            .summary
            .clone()
            .or_else(|| path_item.summary.clone()),
        description: operation
            .description
            .clone()
            .or_else(|| path_item.description.clone()),
        tags: operation.tags.clone(),
        parameters: parameters.into_values().collect(),
        request_body,
        responses,
        examples: Vec::new(),
        auth,
    };
    endpoint.examples = synthesize_examples(&endpoint);
    Ok(endpoint)
}

/// Walk the security requirement list and return the first entry we can
/// faithfully express as an `AuthRequirement`. Handles the common cases
/// (HTTP bearer, HTTP basic, apiKey-in-header, OAuth2) and silently skips
/// requirements that place the credential elsewhere (query, cookie) or
/// schemes we can't encode (OpenID Connect discovery flows, mTLS).
fn resolve_auth_hint(
    requirements: &[SecurityRequirement],
    components: Option<&Components>,
) -> Option<AuthRequirement> {
    let components = components?;
    for requirement in requirements {
        for scheme_name in requirement.keys() {
            let scheme_ref = components.security_schemes.get(scheme_name)?;
            let scheme = scheme_ref.as_item()?;
            if let Some(hint) = security_scheme_to_hint(scheme) {
                return Some(hint);
            }
        }
    }
    None
}

fn security_scheme_to_hint(scheme: &SecurityScheme) -> Option<AuthRequirement> {
    match scheme {
        SecurityScheme::HTTP {
            scheme: http_scheme,
            description,
            ..
        } => {
            let lower = http_scheme.to_ascii_lowercase();
            if lower == "bearer" {
                Some(AuthRequirement {
                    scheme: AuthScheme::HttpBearer,
                    header_name: "Authorization".to_string(),
                    value_prefix: Some("Bearer ".to_string()),
                    description: description.clone(),
                })
            } else if lower == "basic" {
                Some(AuthRequirement {
                    scheme: AuthScheme::HttpBasic,
                    header_name: "Authorization".to_string(),
                    value_prefix: Some("Basic ".to_string()),
                    description: description.clone(),
                })
            } else {
                Some(AuthRequirement {
                    scheme: AuthScheme::Other,
                    header_name: "Authorization".to_string(),
                    value_prefix: None,
                    description: Some(format!("HTTP {http_scheme} auth")),
                })
            }
        }
        SecurityScheme::APIKey {
            location: APIKeyLocation::Header,
            name,
            description,
            ..
        } => Some(AuthRequirement {
            scheme: AuthScheme::ApiKeyHeader,
            header_name: name.clone(),
            value_prefix: None,
            description: description.clone(),
        }),
        // API keys in query / cookie can't be expressed as a header gate.
        // Surface them as `Other` so the UI still explains the ask.
        SecurityScheme::APIKey { location, name, .. } => Some(AuthRequirement {
            scheme: AuthScheme::Other,
            header_name: name.clone(),
            value_prefix: None,
            description: Some(format!("API key in {location:?}")),
        }),
        SecurityScheme::OAuth2 { description, .. } => Some(AuthRequirement {
            scheme: AuthScheme::OAuth2,
            header_name: "Authorization".to_string(),
            value_prefix: Some("Bearer ".to_string()),
            description: description.clone(),
        }),
        SecurityScheme::OpenIDConnect { description, .. } => Some(AuthRequirement {
            scheme: AuthScheme::Other,
            header_name: "Authorization".to_string(),
            value_prefix: Some("Bearer ".to_string()),
            description: description
                .clone()
                .or_else(|| Some("OpenID Connect".to_string())),
        }),
    }
}

fn responses_to_canonical(
    responses: &indexmap::IndexMap<StatusCode, ReferenceOr<Response>>,
    components: Option<&Components>,
    raw_responses: &Value,
    raw_spec: &Value,
) -> Result<Vec<CanonicalResponse>, ParseError> {
    let mut parsed = Vec::new();

    for (status_code, response) in responses {
        let resolved = resolve_response(response, components)?;
        let (content_type, media_type) = select_media_type(&resolved.content);
        let status_key = status_code.to_string();
        let raw_schema = raw_responses
            .get(&status_key)
            .or_else(|| raw_responses.get(status_key.trim_matches('"')))
            .or_else(|| raw_responses.get("default"))
            .map(|response| resolve_raw_reference_if_needed(response, raw_spec))
            .and_then(|response| raw_media_type_schema(response, &content_type, raw_spec));
        parsed.push(CanonicalResponse {
            status_code: status_code.to_string(),
            description: Some(resolved.description.clone()),
            content_type,
            schema: media_type.and_then(|media| {
                let mut schema = media_type_schema(media, components);
                if let (Some(schema), Some(raw_schema)) = (schema.as_mut(), raw_schema) {
                    apply_raw_schema_extensions(schema, raw_schema, raw_spec);
                }
                schema
            }),
        });
    }

    if parsed.is_empty() {
        parsed.push(CanonicalResponse {
            status_code: "200".to_string(),
            description: Some("Default response placeholder".to_string()),
            content_type: "application/json".to_string(),
            schema: None,
        });
    }

    Ok(parsed)
}

fn request_body_to_canonical(
    request_body: &ReferenceOr<RequestBody>,
    components: Option<&Components>,
    raw_request_body: Option<&Value>,
    raw_spec: &Value,
) -> Result<CanonicalRequestBody, ParseError> {
    let resolved = resolve_request_body(request_body, components)?;
    let (content_type, media_type) = select_media_type(&resolved.content);
    let raw_request_body = raw_request_body
        .map(|body| resolve_raw_reference_if_needed(body, raw_spec))
        .unwrap_or(&Value::Null);
    let raw_schema = raw_media_type_schema(raw_request_body, &content_type, raw_spec);

    Ok(CanonicalRequestBody {
        content_type,
        required: resolved.required,
        schema: {
            let mut schema = media_type
                .and_then(|media| media_type_schema(media, components))
                .unwrap_or_else(SchemaNode::object);
            if let Some(raw_schema) = raw_schema {
                apply_raw_schema_extensions(&mut schema, raw_schema, raw_spec);
            }
            schema
        },
    })
}

fn parameter_to_canonical(
    parameter: &ReferenceOr<Parameter>,
    components: Option<&Components>,
) -> Result<CanonicalParameter, ParseError> {
    let resolved = resolve_parameter(parameter, components)?;
    let data = resolved.parameter_data_ref();

    let (location, schema) = match &resolved {
        Parameter::Query { parameter_data, .. } => (
            ParameterLocation::Query,
            parameter_schema_to_node(&parameter_data.format, components),
        ),
        Parameter::Header { parameter_data, .. } => (
            ParameterLocation::Header,
            parameter_schema_to_node(&parameter_data.format, components),
        ),
        Parameter::Path { parameter_data, .. } => (
            ParameterLocation::Path,
            parameter_schema_to_node(&parameter_data.format, components),
        ),
        Parameter::Cookie { parameter_data, .. } => (
            ParameterLocation::Cookie,
            parameter_schema_to_node(&parameter_data.format, components),
        ),
    };

    let mut schema = schema?;
    schema.required = matches!(location, ParameterLocation::Path) || data.required;

    Ok(CanonicalParameter {
        name: data.name.clone(),
        location,
        description: data.description.clone(),
        required: schema.required,
        schema,
    })
}

fn parameter_schema_to_node(
    format: &ParameterSchemaOrContent,
    components: Option<&Components>,
) -> Result<SchemaNode, ParseError> {
    match format {
        ParameterSchemaOrContent::Schema(schema) => Ok(schema_ref_to_node(schema, components)),
        ParameterSchemaOrContent::Content(content) => Ok(select_media_type(content)
            .1
            .and_then(|media| media_type_schema(media, components))
            .unwrap_or_else(SchemaNode::string)),
    }
}

fn media_type_schema(
    media_type: &MediaType,
    components: Option<&Components>,
) -> Option<SchemaNode> {
    media_type
        .schema
        .as_ref()
        .map(|schema| schema_ref_to_node(schema, components))
        .or_else(|| media_type.example.as_ref().map(schema_from_json_value))
}

fn raw_media_type_schema<'a>(
    raw_container: &'a Value,
    content_type: &str,
    raw_spec: &'a Value,
) -> Option<&'a Value> {
    let content = raw_container.get("content")?;
    let media = content
        .get(content_type)
        .or_else(|| content.get("application/json"))
        .or_else(|| {
            content
                .as_object()
                .and_then(|object| object.values().next())
        })?;
    media
        .get("schema")
        .map(|schema| resolve_raw_reference_if_needed(schema, raw_spec))
}

fn resolve_raw_reference_if_needed<'a>(value: &'a Value, raw_spec: &'a Value) -> &'a Value {
    let Some(reference) = value.get("$ref").and_then(Value::as_str) else {
        return value;
    };
    resolve_raw_pointer(reference, raw_spec).unwrap_or(value)
}

fn resolve_raw_pointer<'a>(reference: &str, raw_spec: &'a Value) -> Option<&'a Value> {
    let pointer = reference.strip_prefix('#')?;
    if pointer.is_empty() {
        return Some(raw_spec);
    }
    raw_spec.pointer(pointer)
}

fn apply_raw_schema_extensions(node: &mut SchemaNode, raw_schema: &Value, raw_spec: &Value) {
    let raw_schema = resolve_raw_reference_if_needed(raw_schema, raw_spec);
    apply_raw_conditional_schema_extensions(node, raw_schema, raw_spec);
    match node.node_type {
        SchemaNodeType::Object => apply_raw_object_schema_extensions(node, raw_schema, raw_spec),
        SchemaNodeType::Array => apply_raw_array_schema_extensions(node, raw_schema, raw_spec),
        _ => {}
    }
}

fn apply_raw_conditional_schema_extensions(
    node: &mut SchemaNode,
    raw_schema: &Value,
    raw_spec: &Value,
) {
    if let Some(raw_if) = raw_schema.get("if") {
        let raw_if = resolve_raw_reference_if_needed(raw_if, raw_spec);
        node.if_schema = Some(Box::new(raw_schema_value_to_node(raw_if, raw_spec)));
    }
    if let Some(raw_then) = raw_schema.get("then") {
        let raw_then = resolve_raw_reference_if_needed(raw_then, raw_spec);
        node.then_schema = Some(Box::new(raw_schema_value_to_node(raw_then, raw_spec)));
    }
    if let Some(raw_else) = raw_schema.get("else") {
        let raw_else = resolve_raw_reference_if_needed(raw_else, raw_spec);
        node.else_schema = Some(Box::new(raw_schema_value_to_node(raw_else, raw_spec)));
    }
}

fn apply_raw_object_schema_extensions(node: &mut SchemaNode, raw_schema: &Value, raw_spec: &Value) {
    match raw_schema.get("additionalProperties") {
        Some(Value::Bool(false)) => {
            node.allow_additional_properties = false;
            node.additional_properties = None;
        }
        Some(Value::Bool(true)) => {
            node.allow_additional_properties = true;
            node.additional_properties = None;
        }
        Some(raw_additional) => {
            let raw_additional = resolve_raw_reference_if_needed(raw_additional, raw_spec);
            node.allow_additional_properties = true;
            node.additional_properties =
                Some(Box::new(raw_schema_value_to_node(raw_additional, raw_spec)));
        }
        None => {}
    }
    if raw_schema
        .get("unevaluatedProperties")
        .and_then(Value::as_bool)
        == Some(false)
    {
        node.allow_unevaluated_properties = false;
    }
    if let Some(map) = raw_schema
        .get("dependentRequired")
        .and_then(Value::as_object)
    {
        for (name, value) in map {
            let dependents = value
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect::<Vec<_>>();
            if !dependents.is_empty() {
                node.dependent_required.insert(name.clone(), dependents);
            }
        }
    }
    if let Some(map) = raw_schema
        .get("dependentSchemas")
        .and_then(Value::as_object)
    {
        for (name, raw_dependent) in map {
            let raw_dependent = resolve_raw_reference_if_needed(raw_dependent, raw_spec);
            let mut dependent = raw_schema_value_to_node(raw_dependent, raw_spec);
            apply_raw_schema_extensions(&mut dependent, raw_dependent, raw_spec);
            node.dependent_schemas.insert(name.clone(), dependent);
        }
    }
    if let Some(raw_properties) = raw_schema.get("properties").and_then(Value::as_object) {
        for (name, child) in &mut node.properties {
            if let Some(raw_child) = raw_properties.get(name) {
                apply_raw_schema_extensions(child, raw_child, raw_spec);
            }
        }
    }
}

fn apply_raw_array_schema_extensions(node: &mut SchemaNode, raw_schema: &Value, raw_spec: &Value) {
    if let Some(raw_prefix_items) = raw_schema.get("prefixItems").and_then(Value::as_array) {
        node.prefix_items = raw_prefix_items
            .iter()
            .map(|raw_item| {
                let raw_item = resolve_raw_reference_if_needed(raw_item, raw_spec);
                let mut item = raw_schema_value_to_node(raw_item, raw_spec);
                apply_raw_schema_extensions(&mut item, raw_item, raw_spec);
                item
            })
            .collect();
    }
    if raw_schema.get("unevaluatedItems").and_then(Value::as_bool) == Some(false) {
        node.allow_unevaluated_items = false;
    }
    if let Some(raw_contains) = raw_schema.get("contains") {
        let raw_contains = resolve_raw_reference_if_needed(raw_contains, raw_spec);
        let mut contains = raw_schema_value_to_node(raw_contains, raw_spec);
        apply_raw_schema_extensions(&mut contains, raw_contains, raw_spec);
        node.contains = Some(Box::new(contains));
    }
    node.min_contains = raw_schema
        .get("minContains")
        .and_then(Value::as_u64)
        .map(|value| value as usize);
    node.max_contains = raw_schema
        .get("maxContains")
        .and_then(Value::as_u64)
        .map(|value| value as usize);
    if let Some(raw_items) = raw_schema.get("items") {
        let raw_items = resolve_raw_reference_if_needed(raw_items, raw_spec);
        if raw_items.is_boolean() {
            node.items = Some(Box::new(raw_schema_value_to_node(raw_items, raw_spec)));
        } else if let Some(item) = node.items.as_mut() {
            apply_raw_schema_extensions(item, raw_items, raw_spec);
        }
    }
}

fn raw_schema_value_to_node(raw_schema: &Value, raw_spec: &Value) -> SchemaNode {
    let raw_schema = resolve_raw_reference_if_needed(raw_schema, raw_spec);
    if let Some(value) = raw_schema.as_bool() {
        return SchemaNode::bool_schema(value);
    }
    let mut node = match raw_schema.get("type").and_then(Value::as_str) {
        Some("object") => raw_object_schema_to_node(raw_schema, raw_spec),
        None if raw_schema.get("properties").is_some() => {
            raw_object_schema_to_node(raw_schema, raw_spec)
        }
        Some("array") => raw_array_schema_to_node(raw_schema, raw_spec),
        None if raw_schema.get("items").is_some() || raw_schema.get("contains").is_some() => {
            raw_array_schema_to_node(raw_schema, raw_spec)
        }
        Some("string") => {
            let mut node = SchemaNode::string();
            node.format = raw_schema
                .get("format")
                .and_then(Value::as_str)
                .map(str::to_string);
            node.pattern = raw_schema
                .get("pattern")
                .and_then(Value::as_str)
                .map(str::to_string);
            node.min_length = raw_schema
                .get("minLength")
                .and_then(Value::as_u64)
                .map(|value| value as usize);
            node.max_length = raw_schema
                .get("maxLength")
                .and_then(Value::as_u64)
                .map(|value| value as usize);
            node
        }
        Some("integer") => SchemaNode {
            node_type: SchemaNodeType::Integer,
            minimum: raw_schema.get("minimum").and_then(Value::as_f64),
            maximum: raw_schema.get("maximum").and_then(Value::as_f64),
            multiple_of: raw_schema.get("multipleOf").and_then(Value::as_f64),
            ..SchemaNode::string()
        },
        Some("number") => SchemaNode {
            node_type: SchemaNodeType::Number,
            minimum: raw_schema.get("minimum").and_then(Value::as_f64),
            maximum: raw_schema.get("maximum").and_then(Value::as_f64),
            multiple_of: raw_schema.get("multipleOf").and_then(Value::as_f64),
            ..SchemaNode::string()
        },
        Some("boolean") => SchemaNode {
            node_type: SchemaNodeType::Boolean,
            ..SchemaNode::string()
        },
        Some("null") => SchemaNode {
            node_type: SchemaNodeType::Null,
            nullable: true,
            ..SchemaNode::string()
        },
        _ => SchemaNode {
            node_type: SchemaNodeType::Unknown,
            ..SchemaNode::string()
        },
    };
    node.description = raw_schema
        .get("description")
        .and_then(Value::as_str)
        .map(str::to_string);
    node.enum_values = raw_schema
        .get("enum")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    node.example = raw_schema.get("example").cloned();
    node.nullable = raw_schema
        .get("nullable")
        .and_then(Value::as_bool)
        .unwrap_or(node.nullable);
    node.exclusive_minimum = raw_schema
        .get("exclusiveMinimum")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    node.exclusive_maximum = raw_schema
        .get("exclusiveMaximum")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    node
}

fn raw_object_schema_to_node(raw_schema: &Value, raw_spec: &Value) -> SchemaNode {
    let mut node = SchemaNode::object();
    node.min_properties = raw_schema
        .get("minProperties")
        .and_then(Value::as_u64)
        .map(|value| value as usize);
    node.max_properties = raw_schema
        .get("maxProperties")
        .and_then(Value::as_u64)
        .map(|value| value as usize);
    match raw_schema.get("additionalProperties") {
        Some(Value::Bool(false)) => {
            node.allow_additional_properties = false;
        }
        Some(Value::Bool(true)) | None => {}
        Some(raw_additional) => {
            let raw_additional = resolve_raw_reference_if_needed(raw_additional, raw_spec);
            node.additional_properties =
                Some(Box::new(raw_schema_value_to_node(raw_additional, raw_spec)));
        }
    }
    if raw_schema
        .get("unevaluatedProperties")
        .and_then(Value::as_bool)
        == Some(false)
    {
        node.allow_unevaluated_properties = false;
    }
    let required = raw_schema
        .get("required")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect::<BTreeSet<_>>();
    if let Some(properties) = raw_schema.get("properties").and_then(Value::as_object) {
        for (name, raw_child) in properties {
            let mut child = raw_schema_value_to_node(raw_child, raw_spec);
            child.required = required.contains(name.as_str());
            node.properties.insert(name.clone(), child);
        }
    }
    apply_raw_object_schema_extensions(&mut node, raw_schema, raw_spec);
    node
}

fn raw_array_schema_to_node(raw_schema: &Value, raw_spec: &Value) -> SchemaNode {
    let item = raw_schema
        .get("items")
        .map(|items| raw_schema_value_to_node(items, raw_spec))
        .unwrap_or_else(|| SchemaNode::bool_schema(true));
    let mut node = SchemaNode::array(item);
    node.min_items = raw_schema
        .get("minItems")
        .and_then(Value::as_u64)
        .map(|value| value as usize);
    node.max_items = raw_schema
        .get("maxItems")
        .and_then(Value::as_u64)
        .map(|value| value as usize);
    node.unique_items = raw_schema
        .get("uniqueItems")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    apply_raw_array_schema_extensions(&mut node, raw_schema, raw_spec);
    node
}

fn schema_ref_to_node(schema: &ReferenceOr<Schema>, components: Option<&Components>) -> SchemaNode {
    match schema {
        ReferenceOr::Item(schema) => schema_to_node(schema, components),
        ReferenceOr::Reference { reference } => resolve_schema_reference(reference, components)
            .map(|schema| schema_to_node(schema, components))
            .unwrap_or_else(|| SchemaNode {
                node_type: SchemaNodeType::Unknown,
                description: Some(format!("Unresolved schema reference: {reference}")),
                ..SchemaNode::string()
            }),
    }
}

fn boxed_schema_ref_to_node(
    schema: &ReferenceOr<Box<Schema>>,
    components: Option<&Components>,
) -> SchemaNode {
    match schema {
        ReferenceOr::Item(schema) => schema_to_node(schema, components),
        ReferenceOr::Reference { reference } => resolve_schema_reference(reference, components)
            .map(|schema| schema_to_node(schema, components))
            .unwrap_or_else(|| SchemaNode {
                node_type: SchemaNodeType::Unknown,
                description: Some(format!("Unresolved schema reference: {reference}")),
                ..SchemaNode::string()
            }),
    }
}

fn schema_to_node(schema: &Schema, components: Option<&Components>) -> SchemaNode {
    let mut node = match &schema.schema_kind {
        SchemaKind::Type(Type::String(string_type)) => {
            let mut node = SchemaNode::string();
            node.description = schema.schema_data.description.clone();
            node.nullable = schema.schema_data.nullable;
            node.enum_values = string_type
                .enumeration
                .iter()
                .flatten()
                .cloned()
                .map(Value::String)
                .collect();
            node.example = schema.schema_data.example.clone();
            node.format = format_name(&string_type.format);
            node.pattern = string_type.pattern.clone();
            node.min_length = string_type.min_length;
            node.max_length = string_type.max_length;
            node
        }
        SchemaKind::Type(Type::Number(number_type)) => SchemaNode {
            node_type: SchemaNodeType::Number,
            description: schema.schema_data.description.clone(),
            nullable: schema.schema_data.nullable,
            enum_values: number_type
                .enumeration
                .iter()
                .flatten()
                .filter_map(|value| serde_json::Number::from_f64(*value))
                .map(Value::Number)
                .collect(),
            example: schema.schema_data.example.clone(),
            format: format_name(&number_type.format),
            minimum: number_type.minimum,
            maximum: number_type.maximum,
            exclusive_minimum: number_type.exclusive_minimum,
            exclusive_maximum: number_type.exclusive_maximum,
            multiple_of: number_type.multiple_of,
            ..SchemaNode::string()
        },
        SchemaKind::Type(Type::Integer(integer_type)) => SchemaNode {
            node_type: SchemaNodeType::Integer,
            description: schema.schema_data.description.clone(),
            nullable: schema.schema_data.nullable,
            enum_values: integer_type
                .enumeration
                .iter()
                .flatten()
                .cloned()
                .map(serde_json::Number::from)
                .map(Value::Number)
                .collect(),
            example: schema.schema_data.example.clone(),
            format: format_name(&integer_type.format),
            minimum: integer_type.minimum.map(|value| value as f64),
            maximum: integer_type.maximum.map(|value| value as f64),
            exclusive_minimum: integer_type.exclusive_minimum,
            exclusive_maximum: integer_type.exclusive_maximum,
            multiple_of: integer_type.multiple_of.map(|value| value as f64),
            ..SchemaNode::string()
        },
        SchemaKind::Type(Type::Boolean(boolean_type)) => SchemaNode {
            node_type: SchemaNodeType::Boolean,
            description: schema.schema_data.description.clone(),
            nullable: schema.schema_data.nullable,
            enum_values: boolean_type
                .enumeration
                .iter()
                .flatten()
                .cloned()
                .map(Value::Bool)
                .collect(),
            example: schema.schema_data.example.clone(),
            ..SchemaNode::string()
        },
        SchemaKind::Type(Type::Object(object_type)) => {
            object_type_to_node(object_type, schema, components)
        }
        SchemaKind::Type(Type::Array(array_type)) => SchemaNode {
            node_type: SchemaNodeType::Array,
            description: schema.schema_data.description.clone(),
            nullable: schema.schema_data.nullable,
            items: array_type
                .items
                .as_ref()
                .map(|items| Box::new(boxed_schema_ref_to_node(items, components))),
            example: schema.schema_data.example.clone(),
            min_items: array_type.min_items,
            max_items: array_type.max_items,
            unique_items: array_type.unique_items,
            ..SchemaNode::string()
        },
        SchemaKind::OneOf { one_of } => collapse_variants(one_of, components),
        SchemaKind::AllOf { all_of } => merge_all_of_nodes(all_of, components),
        SchemaKind::AnyOf { any_of } => collapse_variants(any_of, components),
        SchemaKind::Not { .. } => SchemaNode {
            node_type: SchemaNodeType::Unknown,
            description: schema.schema_data.description.clone(),
            nullable: schema.schema_data.nullable,
            example: schema.schema_data.example.clone(),
            ..SchemaNode::string()
        },
        SchemaKind::Any(any_schema) => any_schema_to_node(any_schema, schema, components),
    };

    if node.description.is_none() {
        node.description = schema.schema_data.description.clone();
    }
    if node.example.is_none() {
        node.example = schema.schema_data.example.clone();
    }
    node.nullable = schema.schema_data.nullable;
    node
}

fn object_type_to_node(
    object_type: &ObjectType,
    schema: &Schema,
    components: Option<&Components>,
) -> SchemaNode {
    let required: BTreeSet<&String> = object_type.required.iter().collect();
    let mut node = SchemaNode::object();
    node.description = schema.schema_data.description.clone();
    node.nullable = schema.schema_data.nullable;
    node.example = schema.schema_data.example.clone();
    node.min_properties = object_type.min_properties;
    node.max_properties = object_type.max_properties;
    apply_additional_properties(&mut node, &object_type.additional_properties, components);

    for (name, property_schema) in &object_type.properties {
        let mut child = boxed_schema_ref_to_node(property_schema, components);
        child.required = required.contains(name);
        node.properties.insert(name.clone(), child);
    }

    node
}

fn any_schema_to_node(
    any_schema: &openapiv3::AnySchema,
    schema: &Schema,
    components: Option<&Components>,
) -> SchemaNode {
    if !any_schema.properties.is_empty() {
        let mut node = SchemaNode::object();
        let required: BTreeSet<&String> = any_schema.required.iter().collect();
        apply_additional_properties(&mut node, &any_schema.additional_properties, components);
        for (name, property_schema) in &any_schema.properties {
            let mut child = boxed_schema_ref_to_node(property_schema, components);
            child.required = required.contains(name);
            node.properties.insert(name.clone(), child);
        }
        node.description = schema.schema_data.description.clone();
        node.example = schema.schema_data.example.clone();
        node.min_properties = any_schema.min_properties;
        node.max_properties = any_schema.max_properties;
        node.unique_items = any_schema.unique_items.unwrap_or(false);
        return node;
    }

    if let Some(items) = &any_schema.items {
        let mut node = SchemaNode::array(boxed_schema_ref_to_node(items, components));
        node.description = schema.schema_data.description.clone();
        node.example = schema.schema_data.example.clone();
        node.min_items = any_schema.min_items;
        node.max_items = any_schema.max_items;
        node.unique_items = any_schema.unique_items.unwrap_or(false);
        return node;
    }

    match any_schema.typ.as_deref() {
        Some("null") => SchemaNode {
            node_type: SchemaNodeType::Null,
            description: schema.schema_data.description.clone(),
            nullable: true,
            example: schema.schema_data.example.clone(),
            ..SchemaNode::string()
        },
        Some("string") => {
            let mut node = SchemaNode::string();
            node.description = schema.schema_data.description.clone();
            node.nullable = schema.schema_data.nullable;
            node.enum_values = any_schema.enumeration.clone();
            node.example = schema.schema_data.example.clone();
            node.format = any_schema.format.clone();
            node.pattern = any_schema.pattern.clone();
            node.min_length = any_schema.min_length;
            node.max_length = any_schema.max_length;
            node
        }
        Some("integer") => SchemaNode {
            node_type: SchemaNodeType::Integer,
            description: schema.schema_data.description.clone(),
            nullable: schema.schema_data.nullable,
            enum_values: any_schema.enumeration.clone(),
            example: schema.schema_data.example.clone(),
            format: any_schema.format.clone(),
            minimum: any_schema.minimum,
            maximum: any_schema.maximum,
            exclusive_minimum: any_schema.exclusive_minimum.unwrap_or(false),
            exclusive_maximum: any_schema.exclusive_maximum.unwrap_or(false),
            multiple_of: any_schema.multiple_of,
            ..SchemaNode::string()
        },
        Some("number") => SchemaNode {
            node_type: SchemaNodeType::Number,
            description: schema.schema_data.description.clone(),
            nullable: schema.schema_data.nullable,
            enum_values: any_schema.enumeration.clone(),
            example: schema.schema_data.example.clone(),
            format: any_schema.format.clone(),
            minimum: any_schema.minimum,
            maximum: any_schema.maximum,
            exclusive_minimum: any_schema.exclusive_minimum.unwrap_or(false),
            exclusive_maximum: any_schema.exclusive_maximum.unwrap_or(false),
            multiple_of: any_schema.multiple_of,
            ..SchemaNode::string()
        },
        Some("boolean") => SchemaNode {
            node_type: SchemaNodeType::Boolean,
            description: schema.schema_data.description.clone(),
            nullable: schema.schema_data.nullable,
            enum_values: any_schema.enumeration.clone(),
            example: schema.schema_data.example.clone(),
            ..SchemaNode::string()
        },
        _ => SchemaNode {
            node_type: SchemaNodeType::Unknown,
            description: schema.schema_data.description.clone(),
            nullable: schema.schema_data.nullable,
            enum_values: any_schema.enumeration.clone(),
            example: schema.schema_data.example.clone(),
            ..SchemaNode::string()
        },
    }
}

fn apply_additional_properties(
    node: &mut SchemaNode,
    additional: &Option<AdditionalProperties>,
    components: Option<&Components>,
) {
    match additional {
        Some(AdditionalProperties::Any(false)) => {
            node.allow_additional_properties = false;
            node.additional_properties = None;
        }
        Some(AdditionalProperties::Any(true)) | None => {
            node.allow_additional_properties = true;
            node.additional_properties = None;
        }
        Some(AdditionalProperties::Schema(schema)) => {
            node.allow_additional_properties = true;
            node.additional_properties = Some(Box::new(schema_ref_to_node(schema, components)));
        }
    }
}

fn format_name<T: std::fmt::Debug>(format: &VariantOrUnknownOrEmpty<T>) -> Option<String> {
    match format {
        VariantOrUnknownOrEmpty::Item(value) => {
            let raw = format!("{value:?}");
            Some(
                raw.chars()
                    .enumerate()
                    .flat_map(|(idx, ch)| {
                        let needs_dash = idx > 0 && ch.is_ascii_uppercase();
                        let lower = ch.to_ascii_lowercase();
                        if needs_dash {
                            vec!['-', lower]
                        } else {
                            vec![lower]
                        }
                    })
                    .collect(),
            )
        }
        VariantOrUnknownOrEmpty::Unknown(value) => Some(value.clone()),
        VariantOrUnknownOrEmpty::Empty => None,
    }
}

/// Fold a `oneOf` / `anyOf` branch list into a single canonical node.
///
/// Strategy (keeps the downstream synthesis + UI simple):
/// 1. Any `null`-typed branch marks the result as `nullable`. The remaining
///    branches are still considered for shape selection.
/// 2. If every remaining branch is an object, merge their property maps
///    (union). Conflicting properties on overlapping keys prefer the first
///    occurrence so schema order is meaningful.
/// 3. Otherwise fall back to the first non-null branch.
fn collapse_variants(
    variants: &[ReferenceOr<Schema>],
    components: Option<&Components>,
) -> SchemaNode {
    let nodes: Vec<SchemaNode> = variants
        .iter()
        .map(|schema| schema_ref_to_node(schema, components))
        .collect();

    let (null_branches, concrete_branches): (Vec<_>, Vec<_>) = nodes
        .into_iter()
        .partition(|node| matches!(node.node_type, SchemaNodeType::Null));

    let mut base = if concrete_branches.is_empty() {
        SchemaNode {
            node_type: SchemaNodeType::Unknown,
            description: None,
            example: None,
            ..SchemaNode::string()
        }
    } else if concrete_branches
        .iter()
        .all(|node| matches!(node.node_type, SchemaNodeType::Object))
    {
        let mut merged = SchemaNode::object();
        for branch in &concrete_branches {
            merged.description = merged.description.or(branch.description.clone());
            for (key, value) in &branch.properties {
                merged
                    .properties
                    .entry(key.clone())
                    .or_insert_with(|| value.clone());
            }
        }
        merged
    } else {
        concrete_branches.into_iter().next().unwrap()
    };

    if !null_branches.is_empty() {
        base.nullable = true;
    }
    base
}

fn merge_all_of_nodes(
    all_of: &[ReferenceOr<Schema>],
    components: Option<&Components>,
) -> SchemaNode {
    let mut merged = SchemaNode::object();

    for schema in all_of {
        let resolved = schema_ref_to_node(schema, components);
        if matches!(resolved.node_type, SchemaNodeType::Object) {
            for (name, child) in resolved.properties {
                merged.properties.insert(name, child);
            }
        }
    }

    merged
}

fn resolve_schema_reference<'a>(
    reference: &str,
    components: Option<&'a Components>,
) -> Option<&'a Schema> {
    let schema_name = reference.strip_prefix("#/components/schemas/")?;
    components?.schemas.get(schema_name)?.as_item()
}

fn resolve_parameter<'a>(
    parameter: &'a ReferenceOr<Parameter>,
    components: Option<&'a Components>,
) -> Result<&'a Parameter, ParseError> {
    match parameter {
        ReferenceOr::Item(parameter) => Ok(parameter),
        ReferenceOr::Reference { reference } => {
            let parameter_name = reference
                .strip_prefix("#/components/parameters/")
                .ok_or_else(|| {
                    ParseError::ParseFailed(format!(
                        "unsupported OpenAPI parameter reference: {reference}"
                    ))
                })?;

            components
                .ok_or_else(|| {
                    ParseError::ParseFailed(format!(
                        "OpenAPI parameter reference requires components: {reference}"
                    ))
                })?
                .parameters
                .get(parameter_name)
                .and_then(ReferenceOr::as_item)
                .ok_or_else(|| {
                    ParseError::ParseFailed(format!(
                        "unresolved OpenAPI parameter reference: {reference}"
                    ))
                })
        }
    }
}

fn resolve_request_body<'a>(
    request_body: &'a ReferenceOr<RequestBody>,
    components: Option<&'a Components>,
) -> Result<&'a RequestBody, ParseError> {
    match request_body {
        ReferenceOr::Item(request_body) => Ok(request_body),
        ReferenceOr::Reference { reference } => {
            let body_name = reference
                .strip_prefix("#/components/requestBodies/")
                .ok_or_else(|| {
                    ParseError::ParseFailed(format!(
                        "unsupported OpenAPI request body reference: {reference}"
                    ))
                })?;

            components
                .ok_or_else(|| {
                    ParseError::ParseFailed(format!(
                        "OpenAPI request body reference requires components: {reference}"
                    ))
                })?
                .request_bodies
                .get(body_name)
                .and_then(ReferenceOr::as_item)
                .ok_or_else(|| {
                    ParseError::ParseFailed(format!(
                        "unresolved OpenAPI request body reference: {reference}"
                    ))
                })
        }
    }
}

fn resolve_response<'a>(
    response: &'a ReferenceOr<Response>,
    components: Option<&'a Components>,
) -> Result<&'a Response, ParseError> {
    match response {
        ReferenceOr::Item(response) => Ok(response),
        ReferenceOr::Reference { reference } => {
            let response_name = reference
                .strip_prefix("#/components/responses/")
                .ok_or_else(|| {
                    ParseError::ParseFailed(format!(
                        "unsupported OpenAPI response reference: {reference}"
                    ))
                })?;

            components
                .ok_or_else(|| {
                    ParseError::ParseFailed(format!(
                        "OpenAPI response reference requires components: {reference}"
                    ))
                })?
                .responses
                .get(response_name)
                .and_then(ReferenceOr::as_item)
                .ok_or_else(|| {
                    ParseError::ParseFailed(format!(
                        "unresolved OpenAPI response reference: {reference}"
                    ))
                })
        }
    }
}

fn select_media_type(
    content: &indexmap::IndexMap<String, MediaType>,
) -> (String, Option<&MediaType>) {
    if let Some(media) = content.get("application/json") {
        return ("application/json".to_string(), Some(media));
    }

    content
        .iter()
        .next()
        .map(|(content_type, media)| (content_type.clone(), Some(media)))
        .unwrap_or_else(|| ("application/json".to_string(), None))
}

fn http_method_from_str(method: &str) -> Result<HttpMethod, ParseError> {
    match method.to_ascii_lowercase().as_str() {
        "get" => Ok(HttpMethod::Get),
        "post" => Ok(HttpMethod::Post),
        "put" => Ok(HttpMethod::Put),
        "patch" => Ok(HttpMethod::Patch),
        "delete" => Ok(HttpMethod::Delete),
        "options" => Ok(HttpMethod::Options),
        "head" => Ok(HttpMethod::Head),
        unsupported => Err(ParseError::ParseFailed(format!(
            "unsupported HTTP method in OpenAPI document: {unsupported}"
        ))),
    }
}

fn parameter_key(parameter: &CanonicalParameter) -> String {
    format!(
        "{}:{}",
        match parameter.location {
            ParameterLocation::Path => "path",
            ParameterLocation::Query => "query",
            ParameterLocation::Header => "header",
            ParameterLocation::Cookie => "cookie",
        },
        parameter.name
    )
}

fn canonical_id(input: &str) -> String {
    let mut normalized = String::new();
    for character in input.chars() {
        if character.is_ascii_alphanumeric() {
            normalized.push(character.to_ascii_lowercase());
        } else if !normalized.ends_with('-') {
            normalized.push('-');
        }
    }

    normalized.trim_matches('-').to_string()
}
