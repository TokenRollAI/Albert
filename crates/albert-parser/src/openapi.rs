use std::collections::{BTreeMap, BTreeSet};

use albert_core::{
    AuthRequirement, AuthScheme, CanonicalApiCollection, CanonicalEndpoint, CanonicalParameter,
    CanonicalRequestBody, CanonicalResponse, HttpMethod, InputSourceKind, ParameterLocation,
    SchemaNode, SchemaNodeType, synthesize_examples,
};
use openapiv3::{
    APIKeyLocation, Components, MediaType, ObjectType, OpenAPI, Operation, Parameter,
    ParameterSchemaOrContent, PathItem, ReferenceOr, RequestBody, Response, Schema, SchemaKind,
    SecurityRequirement, SecurityScheme, StatusCode, Type,
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
        let components = spec.components.as_ref();
        let collection_name = source.name.unwrap_or_else(|| spec.info.title.clone());

        let mut endpoints = Vec::new();
        let default_security = spec.security.as_ref();
        for (path, item_ref) in spec.paths.iter() {
            let Some(path_item) = item_ref.as_item() else {
                continue;
            };

            endpoints.extend(path_item_to_endpoints(
                path,
                path_item,
                components,
                default_security,
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
}

fn path_item_to_endpoints(
    path: &str,
    path_item: &PathItem,
    components: Option<&Components>,
    default_security: Option<&Vec<SecurityRequirement>>,
) -> Result<Vec<CanonicalEndpoint>, ParseError> {
    let mut endpoints = Vec::new();

    for (method, operation) in path_item.iter() {
        endpoints.push(operation_to_endpoint(
            path,
            method,
            path_item,
            operation,
            components,
            default_security,
        )?);
    }

    Ok(endpoints)
}

fn operation_to_endpoint(
    path: &str,
    method: &str,
    path_item: &PathItem,
    operation: &Operation,
    components: Option<&Components>,
    default_security: Option<&Vec<SecurityRequirement>>,
) -> Result<CanonicalEndpoint, ParseError> {
    let mut parameters = BTreeMap::new();

    for parameter in &path_item.parameters {
        let canonical = parameter_to_canonical(parameter, components)?;
        parameters.insert(parameter_key(&canonical), canonical);
    }

    for parameter in &operation.parameters {
        let canonical = parameter_to_canonical(parameter, components)?;
        parameters.insert(parameter_key(&canonical), canonical);
    }

    let request_body = operation
        .request_body
        .as_ref()
        .map(|body| request_body_to_canonical(body, components))
        .transpose()?;

    let responses = responses_to_canonical(&operation.responses.responses, components)?;

    // Operation-level security overrides the top-level default (even when
    // empty — an empty list means "explicitly no auth required").
    let effective_security = operation.security.as_ref().or(default_security);
    let auth = effective_security.and_then(|reqs| resolve_auth_hint(reqs, components));

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
) -> Result<Vec<CanonicalResponse>, ParseError> {
    let mut parsed = Vec::new();

    for (status_code, response) in responses {
        let resolved = resolve_response(response, components)?;
        let (content_type, media_type) = select_media_type(&resolved.content);
        parsed.push(CanonicalResponse {
            status_code: status_code.to_string(),
            description: Some(resolved.description.clone()),
            content_type,
            schema: media_type.and_then(|media| media_type_schema(media, components)),
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
) -> Result<CanonicalRequestBody, ParseError> {
    let resolved = resolve_request_body(request_body, components)?;
    let (content_type, media_type) = select_media_type(&resolved.content);

    Ok(CanonicalRequestBody {
        content_type,
        required: resolved.required,
        schema: media_type
            .and_then(|media| media_type_schema(media, components))
            .unwrap_or_else(SchemaNode::object),
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

fn schema_ref_to_node(schema: &ReferenceOr<Schema>, components: Option<&Components>) -> SchemaNode {
    match schema {
        ReferenceOr::Item(schema) => schema_to_node(schema, components),
        ReferenceOr::Reference { reference } => resolve_schema_reference(reference, components)
            .map(|schema| schema_to_node(schema, components))
            .unwrap_or_else(|| SchemaNode {
                node_type: SchemaNodeType::Unknown,
                description: Some(format!("Unresolved schema reference: {reference}")),
                required: false,
                nullable: false,
                properties: Default::default(),
                items: None,
                enum_values: Vec::new(),
                example: None,
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
                required: false,
                nullable: false,
                properties: Default::default(),
                items: None,
                enum_values: Vec::new(),
                example: None,
            }),
    }
}

fn schema_to_node(schema: &Schema, components: Option<&Components>) -> SchemaNode {
    let mut node = match &schema.schema_kind {
        SchemaKind::Type(Type::String(string_type)) => SchemaNode {
            node_type: SchemaNodeType::String,
            description: schema.schema_data.description.clone(),
            required: false,
            nullable: schema.schema_data.nullable,
            properties: Default::default(),
            items: None,
            enum_values: string_type
                .enumeration
                .iter()
                .flatten()
                .cloned()
                .map(Value::String)
                .collect(),
            example: schema.schema_data.example.clone(),
        },
        SchemaKind::Type(Type::Number(number_type)) => SchemaNode {
            node_type: SchemaNodeType::Number,
            description: schema.schema_data.description.clone(),
            required: false,
            nullable: schema.schema_data.nullable,
            properties: Default::default(),
            items: None,
            enum_values: number_type
                .enumeration
                .iter()
                .flatten()
                .filter_map(|value| serde_json::Number::from_f64(*value))
                .map(Value::Number)
                .collect(),
            example: schema.schema_data.example.clone(),
        },
        SchemaKind::Type(Type::Integer(integer_type)) => SchemaNode {
            node_type: SchemaNodeType::Integer,
            description: schema.schema_data.description.clone(),
            required: false,
            nullable: schema.schema_data.nullable,
            properties: Default::default(),
            items: None,
            enum_values: integer_type
                .enumeration
                .iter()
                .flatten()
                .cloned()
                .map(serde_json::Number::from)
                .map(Value::Number)
                .collect(),
            example: schema.schema_data.example.clone(),
        },
        SchemaKind::Type(Type::Boolean(boolean_type)) => SchemaNode {
            node_type: SchemaNodeType::Boolean,
            description: schema.schema_data.description.clone(),
            required: false,
            nullable: schema.schema_data.nullable,
            properties: Default::default(),
            items: None,
            enum_values: boolean_type
                .enumeration
                .iter()
                .flatten()
                .cloned()
                .map(Value::Bool)
                .collect(),
            example: schema.schema_data.example.clone(),
        },
        SchemaKind::Type(Type::Object(object_type)) => {
            object_type_to_node(object_type, schema, components)
        }
        SchemaKind::Type(Type::Array(array_type)) => SchemaNode {
            node_type: SchemaNodeType::Array,
            description: schema.schema_data.description.clone(),
            required: false,
            nullable: schema.schema_data.nullable,
            properties: Default::default(),
            items: array_type
                .items
                .as_ref()
                .map(|items| Box::new(boxed_schema_ref_to_node(items, components))),
            enum_values: Vec::new(),
            example: schema.schema_data.example.clone(),
        },
        SchemaKind::OneOf { one_of } => collapse_variants(one_of, components),
        SchemaKind::AllOf { all_of } => merge_all_of_nodes(all_of, components),
        SchemaKind::AnyOf { any_of } => collapse_variants(any_of, components),
        SchemaKind::Not { .. } => SchemaNode {
            node_type: SchemaNodeType::Unknown,
            description: schema.schema_data.description.clone(),
            required: false,
            nullable: schema.schema_data.nullable,
            properties: Default::default(),
            items: None,
            enum_values: Vec::new(),
            example: schema.schema_data.example.clone(),
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
        for (name, property_schema) in &any_schema.properties {
            let mut child = boxed_schema_ref_to_node(property_schema, components);
            child.required = required.contains(name);
            node.properties.insert(name.clone(), child);
        }
        node.description = schema.schema_data.description.clone();
        node.example = schema.schema_data.example.clone();
        return node;
    }

    if let Some(items) = &any_schema.items {
        let mut node = SchemaNode::array(boxed_schema_ref_to_node(items, components));
        node.description = schema.schema_data.description.clone();
        node.example = schema.schema_data.example.clone();
        return node;
    }

    match any_schema.typ.as_deref() {
        Some("null") => SchemaNode {
            node_type: SchemaNodeType::Null,
            description: schema.schema_data.description.clone(),
            required: false,
            nullable: true,
            properties: Default::default(),
            items: None,
            enum_values: Vec::new(),
            example: schema.schema_data.example.clone(),
        },
        Some("string") => SchemaNode::string(),
        Some("integer") => SchemaNode {
            node_type: SchemaNodeType::Integer,
            description: schema.schema_data.description.clone(),
            required: false,
            nullable: schema.schema_data.nullable,
            properties: Default::default(),
            items: None,
            enum_values: any_schema.enumeration.clone(),
            example: schema.schema_data.example.clone(),
        },
        Some("number") => SchemaNode {
            node_type: SchemaNodeType::Number,
            description: schema.schema_data.description.clone(),
            required: false,
            nullable: schema.schema_data.nullable,
            properties: Default::default(),
            items: None,
            enum_values: any_schema.enumeration.clone(),
            example: schema.schema_data.example.clone(),
        },
        Some("boolean") => SchemaNode {
            node_type: SchemaNodeType::Boolean,
            description: schema.schema_data.description.clone(),
            required: false,
            nullable: schema.schema_data.nullable,
            properties: Default::default(),
            items: None,
            enum_values: any_schema.enumeration.clone(),
            example: schema.schema_data.example.clone(),
        },
        _ => SchemaNode {
            node_type: SchemaNodeType::Unknown,
            description: schema.schema_data.description.clone(),
            required: false,
            nullable: schema.schema_data.nullable,
            properties: Default::default(),
            items: None,
            enum_values: any_schema.enumeration.clone(),
            example: schema.schema_data.example.clone(),
        },
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
            required: false,
            nullable: false,
            properties: Default::default(),
            items: None,
            enum_values: Vec::new(),
            example: None,
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
