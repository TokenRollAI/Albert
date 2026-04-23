use std::collections::{BTreeMap, BTreeSet};

use albert_core::{
    CanonicalApiCollection, CanonicalEndpoint, CanonicalParameter, CanonicalRequestBody,
    CanonicalResponse, HttpMethod, InputSourceKind, ParameterLocation, SchemaNode, SchemaNodeType,
    default_mock_examples,
};
use openapiv3::{
    Components, MediaType, ObjectType, OpenAPI, Operation, Parameter, ParameterSchemaOrContent,
    PathItem, ReferenceOr, RequestBody, Response, Schema, SchemaKind, StatusCode, Type,
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
        for (path, item_ref) in spec.paths.iter() {
            let Some(path_item) = item_ref.as_item() else {
                continue;
            };

            endpoints.extend(path_item_to_endpoints(path, path_item, components)?);
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
) -> Result<Vec<CanonicalEndpoint>, ParseError> {
    let mut endpoints = Vec::new();

    for (method, operation) in path_item.iter() {
        endpoints.push(operation_to_endpoint(
            path, method, path_item, operation, components,
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

    Ok(CanonicalEndpoint {
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
        examples: default_mock_examples(),
    })
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
        SchemaKind::OneOf { one_of } => one_of
            .first()
            .map(|schema| schema_ref_to_node(schema, components))
            .unwrap_or_else(SchemaNode::object),
        SchemaKind::AllOf { all_of } => merge_all_of_nodes(all_of, components),
        SchemaKind::AnyOf { any_of } => any_of
            .first()
            .map(|schema| schema_ref_to_node(schema, components))
            .unwrap_or_else(SchemaNode::object),
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

fn select_media_type<'a>(
    content: &'a indexmap::IndexMap<String, MediaType>,
) -> (String, Option<&'a MediaType>) {
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
