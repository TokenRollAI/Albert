use std::collections::BTreeMap;

use albert_core::{
    CanonicalApiCollection, CanonicalEndpoint, CanonicalParameter, CanonicalRequestBody,
    CanonicalResponse, MockExample, MockExampleKind, ParameterLocation, SchemaNode,
    synthesize_value, validate_value,
};
use serde::{Deserialize, Serialize};

use crate::services::default_database_url;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ImportEndpointChange {
    pub method: String,
    pub path: String,
    pub summary: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reasons: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub details: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ImportDiffSummary {
    pub added: Vec<ImportEndpointChange>,
    pub removed: Vec<ImportEndpointChange>,
    pub changed: Vec<ImportEndpointChange>,
    pub unchanged: usize,
}

impl ImportDiffSummary {
    fn for_new_import(collection: &CanonicalApiCollection) -> Self {
        Self {
            added: collection.endpoints.iter().map(endpoint_change).collect(),
            removed: Vec::new(),
            changed: Vec::new(),
            unchanged: 0,
        }
    }
}

/// Persist a single or bundled set of collections into the store and return
/// import summaries. Diffs are computed before each collection is overwritten
/// so re-importing an existing source can surface endpoint-level drift.
fn persist_collections(
    store: &albert_storage::SqliteStore,
    collections: &[CanonicalApiCollection],
    database_url: &str,
) -> Result<Vec<ImportResult>, String> {
    store.migrate().map_err(|error| error.to_string())?;
    let mut imported = Vec::with_capacity(collections.len());
    for collection in collections {
        let existing = store
            .load_collection(&collection.id)
            .map_err(|error| error.to_string())?;
        let diff = existing
            .as_ref()
            .map(|previous| diff_collections(previous, collection))
            .unwrap_or_else(|| ImportDiffSummary::for_new_import(collection));
        store
            .save_collection(collection)
            .map_err(|error| error.to_string())?;
        imported.push(ImportResult {
            collection_id: collection.id.clone(),
            collection_name: collection.name.clone(),
            endpoint_count: collection.endpoints.len(),
            database_url: database_url.to_string(),
            diff,
        });
    }
    Ok(imported)
}

#[derive(Debug, Serialize)]
pub struct ImportResult {
    pub collection_id: String,
    pub collection_name: String,
    pub endpoint_count: usize,
    pub database_url: String,
    pub diff: ImportDiffSummary,
}

/// Synthesize a JSON sample for a request body based on the canonical
/// schema. Returns `null` when the endpoint doesn't declare a request body,
/// so the frontend can show a placeholder instead of failing.
#[tauri::command]
pub fn synthesize_request_body(endpoint: CanonicalEndpoint) -> serde_json::Value {
    endpoint
        .request_body
        .as_ref()
        .map(|body| synthesize_value(&body.schema))
        .unwrap_or(serde_json::Value::Null)
}

#[derive(Debug, Clone, Deserialize)]
pub struct ValidateMockPayloadArgs {
    pub schema: SchemaNode,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct ValidateMockPayloadResult {
    pub valid: bool,
    pub errors: Vec<String>,
}

fn endpoint_change(endpoint: &CanonicalEndpoint) -> ImportEndpointChange {
    ImportEndpointChange {
        method: endpoint.method.as_str().to_string(),
        path: endpoint.path.clone(),
        summary: endpoint.summary.clone(),
        reasons: Vec::new(),
        details: Vec::new(),
    }
}

fn changed_endpoint(
    endpoint: &CanonicalEndpoint,
    reasons: Vec<String>,
    details: Vec<String>,
) -> ImportEndpointChange {
    ImportEndpointChange {
        reasons,
        details,
        ..endpoint_change(endpoint)
    }
}

fn endpoint_key(endpoint: &CanonicalEndpoint) -> (String, String) {
    (endpoint.method.as_str().to_string(), endpoint.path.clone())
}

fn comparable_endpoint(endpoint: &CanonicalEndpoint) -> Result<String, serde_json::Error> {
    let mut clone = endpoint.clone();
    clone.examples.clear();
    serde_json::to_string(&clone)
}

fn endpoint_change_reasons(previous: &CanonicalEndpoint, next: &CanonicalEndpoint) -> Vec<String> {
    let mut reasons = Vec::new();
    if previous.operation_id != next.operation_id
        || previous.summary != next.summary
        || previous.description != next.description
        || previous.tags != next.tags
    {
        reasons.push("metadata changed".to_string());
    }
    if previous.parameters != next.parameters {
        reasons.push("parameters changed".to_string());
    }
    if previous.request_body != next.request_body {
        reasons.push("request body changed".to_string());
    }
    if previous.responses != next.responses {
        reasons.push("responses changed".to_string());
    }
    if previous.auth != next.auth {
        reasons.push("auth changed".to_string());
    }
    if reasons.is_empty() {
        reasons.push("endpoint contract changed".to_string());
    }
    reasons
}

fn endpoint_change_details(previous: &CanonicalEndpoint, next: &CanonicalEndpoint) -> Vec<String> {
    let mut details = Vec::new();
    if previous.operation_id != next.operation_id {
        details.push(label_change(
            "operationId",
            previous.operation_id.as_deref(),
            next.operation_id.as_deref(),
        ));
    }
    if previous.summary != next.summary {
        details.push(label_change(
            "summary",
            previous.summary.as_deref(),
            next.summary.as_deref(),
        ));
    }
    if previous.description != next.description {
        details.push("description changed".to_string());
    }
    if previous.tags != next.tags {
        details.push(format!(
            "tags changed: {} -> {}",
            list_or_dash(&previous.tags),
            list_or_dash(&next.tags)
        ));
    }
    diff_parameters(&previous.parameters, &next.parameters, &mut details);
    diff_request_body(
        previous.request_body.as_ref(),
        next.request_body.as_ref(),
        &mut details,
    );
    diff_responses(&previous.responses, &next.responses, &mut details);
    if previous.auth != next.auth {
        details.push("auth requirement changed".to_string());
    }
    details
}

fn diff_parameters(
    previous: &[CanonicalParameter],
    next: &[CanonicalParameter],
    details: &mut Vec<String>,
) {
    let previous_by_key = previous
        .iter()
        .map(|parameter| (parameter_key(parameter), parameter))
        .collect::<BTreeMap<_, _>>();
    let next_by_key = next
        .iter()
        .map(|parameter| (parameter_key(parameter), parameter))
        .collect::<BTreeMap<_, _>>();

    for (key, next_parameter) in &next_by_key {
        let Some(previous_parameter) = previous_by_key.get(key) else {
            details.push(format!(
                "parameter added: {}",
                parameter_label(next_parameter)
            ));
            continue;
        };
        if *previous_parameter != *next_parameter {
            details.push(format!(
                "parameter changed: {}",
                parameter_label(next_parameter)
            ));
        }
    }
    for (key, previous_parameter) in &previous_by_key {
        if !next_by_key.contains_key(key) {
            details.push(format!(
                "parameter removed: {}",
                parameter_label(previous_parameter)
            ));
        }
    }
}

fn diff_request_body(
    previous: Option<&CanonicalRequestBody>,
    next: Option<&CanonicalRequestBody>,
    details: &mut Vec<String>,
) {
    match (previous, next) {
        (None, None) => {}
        (None, Some(next_body)) => {
            details.push(format!(
                "request body added: {}",
                request_body_label(next_body)
            ));
        }
        (Some(previous_body), None) => {
            details.push(format!(
                "request body removed: {}",
                request_body_label(previous_body)
            ));
        }
        (Some(previous_body), Some(next_body)) => {
            if previous_body.content_type != next_body.content_type {
                details.push(format!(
                    "request body content type changed: {} -> {}",
                    previous_body.content_type, next_body.content_type
                ));
            }
            if previous_body.required != next_body.required {
                details.push(format!(
                    "request body required changed: {} -> {}",
                    previous_body.required, next_body.required
                ));
            }
            if previous_body.schema != next_body.schema {
                details.push("request body schema changed".to_string());
            }
        }
    }
}

fn diff_responses(
    previous: &[CanonicalResponse],
    next: &[CanonicalResponse],
    details: &mut Vec<String>,
) {
    let previous_by_status = previous
        .iter()
        .map(|response| (response.status_code.clone(), response))
        .collect::<BTreeMap<_, _>>();
    let next_by_status = next
        .iter()
        .map(|response| (response.status_code.clone(), response))
        .collect::<BTreeMap<_, _>>();

    for (status, next_response) in &next_by_status {
        let Some(previous_response) = previous_by_status.get(status) else {
            details.push(format!("response added: {}", response_label(next_response)));
            continue;
        };
        if *previous_response != *next_response {
            let mut parts = Vec::new();
            if previous_response.content_type != next_response.content_type {
                parts.push(format!(
                    "content type {} -> {}",
                    previous_response.content_type, next_response.content_type
                ));
            }
            if previous_response.description != next_response.description {
                parts.push("description".to_string());
            }
            if previous_response.schema != next_response.schema {
                parts.push("schema".to_string());
            }
            details.push(format!(
                "response changed: {} ({})",
                status,
                parts.join(", ")
            ));
        }
    }
    for (status, previous_response) in &previous_by_status {
        if !next_by_status.contains_key(status) {
            details.push(format!(
                "response removed: {}",
                response_label(previous_response)
            ));
        }
    }
}

fn parameter_key(parameter: &CanonicalParameter) -> (String, String) {
    (
        location_label(&parameter.location).to_string(),
        parameter.name.clone(),
    )
}

fn parameter_label(parameter: &CanonicalParameter) -> String {
    format!("{} {}", location_label(&parameter.location), parameter.name)
}

fn request_body_label(body: &CanonicalRequestBody) -> String {
    let required = if body.required {
        "required"
    } else {
        "optional"
    };
    format!("{} ({required})", body.content_type)
}

fn response_label(response: &CanonicalResponse) -> String {
    format!("{} {}", response.status_code, response.content_type)
}

fn location_label(location: &ParameterLocation) -> &'static str {
    match location {
        ParameterLocation::Path => "path",
        ParameterLocation::Query => "query",
        ParameterLocation::Header => "header",
        ParameterLocation::Cookie => "cookie",
    }
}

fn label_change(label: &str, previous: Option<&str>, next: Option<&str>) -> String {
    format!(
        "{label} changed: {} -> {}",
        previous.unwrap_or("-"),
        next.unwrap_or("-")
    )
}

fn list_or_dash(values: &[String]) -> String {
    if values.is_empty() {
        "-".to_string()
    } else {
        values.join(", ")
    }
}

fn diff_collections(
    previous: &CanonicalApiCollection,
    next: &CanonicalApiCollection,
) -> ImportDiffSummary {
    let previous_by_key = previous
        .endpoints
        .iter()
        .map(|endpoint| (endpoint_key(endpoint), endpoint))
        .collect::<BTreeMap<_, _>>();
    let next_by_key = next
        .endpoints
        .iter()
        .map(|endpoint| (endpoint_key(endpoint), endpoint))
        .collect::<BTreeMap<_, _>>();

    let mut added = Vec::new();
    let mut removed = Vec::new();
    let mut changed = Vec::new();
    let mut unchanged = 0;

    for (key, next_endpoint) in &next_by_key {
        let Some(previous_endpoint) = previous_by_key.get(key) else {
            added.push(endpoint_change(next_endpoint));
            continue;
        };
        match (
            comparable_endpoint(previous_endpoint),
            comparable_endpoint(next_endpoint),
        ) {
            (Ok(previous_contract), Ok(next_contract)) if previous_contract == next_contract => {
                unchanged += 1;
            }
            _ => changed.push(changed_endpoint(
                next_endpoint,
                endpoint_change_reasons(previous_endpoint, next_endpoint),
                endpoint_change_details(previous_endpoint, next_endpoint),
            )),
        }
    }

    for (key, previous_endpoint) in &previous_by_key {
        if !next_by_key.contains_key(key) {
            removed.push(endpoint_change(previous_endpoint));
        }
    }

    ImportDiffSummary {
        added,
        removed,
        changed,
        unchanged,
    }
}

/// Validate a candidate mock payload with the canonical Rust validator. The
/// frontend uses this before saving captured Try-it responses so mismatch
/// warnings match gateway and AI-generation semantics when Tauri is available.
#[tauri::command]
pub fn validate_mock_payload(args: ValidateMockPayloadArgs) -> ValidateMockPayloadResult {
    let errors = validate_value(&args.schema, &args.payload);
    ValidateMockPayloadResult {
        valid: errors.is_empty(),
        errors,
    }
}

#[tauri::command]
pub fn parse_api_description(
    body: String,
    name: Option<String>,
) -> Result<CanonicalApiCollection, String> {
    albert_parser::parse_source(albert_parser::ParseSource { name, body })
        .map_err(|error| error.to_string())
}

#[derive(Debug, Serialize)]
pub struct BundleImportResult {
    pub database_url: String,
    pub imported: Vec<ImportResult>,
}

#[tauri::command]
pub fn import_api_description(
    body: String,
    name: Option<String>,
    database_url: Option<String>,
) -> Result<ImportResult, String> {
    // Fast path: bundle import. If the body is a JSON array of canonical
    // snapshots we persist every entry and return the first one's summary.
    // For more visibility the caller can invoke `import_bundle` explicitly.
    if let Some(collections) =
        albert_parser::try_parse_bundle(&body).map_err(|error| error.to_string())?
        && let Some(first) = collections.first().cloned()
    {
        let database_url = database_url.unwrap_or_else(default_database_url);
        let store = albert_storage::SqliteStore::new(database_url.clone());
        let mut imported = persist_collections(&store, &collections, &database_url)?;
        if let Some(summary) = imported.first_mut() {
            summary.endpoint_count = collections.iter().map(|c| c.endpoints.len()).sum();
        }
        return imported
            .into_iter()
            .next()
            .ok_or_else(|| "bundle did not contain any collections".to_string())
            .map(|mut result| {
                result.collection_id = first.id;
                result.collection_name = first.name;
                result
            });
    }

    let collection = albert_parser::parse_source(albert_parser::ParseSource { name, body })
        .map_err(|error| error.to_string())?;
    let database_url = database_url.unwrap_or_else(default_database_url);
    let store = albert_storage::SqliteStore::new(database_url.clone());
    persist_collections(&store, std::slice::from_ref(&collection), &database_url)?
        .into_iter()
        .next()
        .ok_or_else(|| "import did not produce a collection summary".to_string())
}

#[tauri::command]
pub fn import_bundle(
    body: String,
    database_url: Option<String>,
) -> Result<BundleImportResult, String> {
    let collections = albert_parser::try_parse_bundle(&body)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "body is not a recognized collection bundle".to_string())?;
    let database_url = database_url.unwrap_or_else(default_database_url);
    let store = albert_storage::SqliteStore::new(database_url.clone());
    let imported = persist_collections(&store, &collections, &database_url)?;
    Ok(BundleImportResult {
        database_url,
        imported,
    })
}

#[tauri::command]
pub fn list_imported_collections(
    database_url: Option<String>,
) -> Result<Vec<albert_storage::StoredCollectionSummary>, String> {
    let store = albert_storage::SqliteStore::new(database_url.unwrap_or_else(default_database_url));
    store.migrate().map_err(|error| error.to_string())?;
    store.list_collections().map_err(|error| error.to_string())
}

#[tauri::command]
pub fn list_imported_endpoints(
    collection_id: String,
    database_url: Option<String>,
) -> Result<Vec<albert_storage::StoredEndpointSummary>, String> {
    let store = albert_storage::SqliteStore::new(database_url.unwrap_or_else(default_database_url));
    store.migrate().map_err(|error| error.to_string())?;
    store
        .list_endpoints(&collection_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn load_collection_snapshot(
    collection_id: String,
    database_url: Option<String>,
) -> Result<Option<CanonicalApiCollection>, String> {
    let store = albert_storage::SqliteStore::new(database_url.unwrap_or_else(default_database_url));
    store.migrate().map_err(|error| error.to_string())?;
    store
        .load_collection(&collection_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn rename_collection(
    collection_id: String,
    new_name: String,
    database_url: Option<String>,
) -> Result<bool, String> {
    if new_name.trim().is_empty() {
        return Err("collection name cannot be empty".into());
    }
    let store = albert_storage::SqliteStore::new(database_url.unwrap_or_else(default_database_url));
    store.migrate().map_err(|error| error.to_string())?;
    store
        .rename_collection(&collection_id, new_name.trim())
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn delete_collection(
    collection_id: String,
    database_url: Option<String>,
) -> Result<bool, String> {
    let store = albert_storage::SqliteStore::new(database_url.unwrap_or_else(default_database_url));
    store.migrate().map_err(|error| error.to_string())?;
    store
        .delete_collection(&collection_id)
        .map_err(|error| error.to_string())
}

#[derive(Debug, Clone, Deserialize)]
pub struct SaveMockExampleArgs {
    pub collection_id: String,
    pub method: String,
    pub path: String,
    pub kind: MockExampleKind,
    pub title: Option<String>,
    pub payload: serde_json::Value,
    pub note: Option<String>,
    #[serde(default)]
    pub database_url: Option<String>,
}

#[tauri::command]
pub fn save_mock_example(args: SaveMockExampleArgs) -> Result<MockExample, String> {
    let store =
        albert_storage::SqliteStore::new(args.database_url.unwrap_or_else(default_database_url));
    store.migrate().map_err(|error| error.to_string())?;
    let kind = args.kind;
    let example = MockExample {
        kind: kind.clone(),
        title: args.title.unwrap_or_else(|| match kind {
            MockExampleKind::Success => "Success".to_string(),
            MockExampleKind::Empty => "Empty".to_string(),
            MockExampleKind::Error => "Error".to_string(),
        }),
        payload: args.payload,
        note: args.note.or_else(|| Some("Hand-edited".to_string())),
    };
    store
        .replace_mock_example(&args.collection_id, &args.method, &args.path, &example)
        .map_err(|error| error.to_string())?;
    Ok(example)
}

#[tauri::command]
pub fn export_collection_json(
    collection_id: String,
    database_url: Option<String>,
) -> Result<String, String> {
    let store = albert_storage::SqliteStore::new(database_url.unwrap_or_else(default_database_url));
    store.migrate().map_err(|error| error.to_string())?;
    let collection = store
        .load_collection(&collection_id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| format!("collection '{collection_id}' not found"))?;
    serde_json::to_string_pretty(&collection).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn export_all_collections_json(database_url: Option<String>) -> Result<String, String> {
    let store = albert_storage::SqliteStore::new(database_url.unwrap_or_else(default_database_url));
    store.migrate().map_err(|error| error.to_string())?;
    let collections = store
        .load_all_collections()
        .map_err(|error| error.to_string())?;
    serde_json::to_string_pretty(&collections).map_err(|err| err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use albert_core::{
        CanonicalApiCollection, CanonicalParameter, CanonicalResponse, HttpMethod, InputSourceKind,
        SchemaNodeType, default_mock_examples,
    };
    use serde_json::json;

    fn endpoint(method: HttpMethod, path: &str, summary: &str) -> CanonicalEndpoint {
        CanonicalEndpoint {
            operation_id: Some(format!("{}{}", method.as_str().to_lowercase(), path)),
            method,
            path: path.to_string(),
            summary: Some(summary.to_string()),
            description: None,
            tags: Vec::new(),
            parameters: Vec::new(),
            request_body: None,
            responses: vec![CanonicalResponse {
                status_code: "200".to_string(),
                description: Some("OK".to_string()),
                content_type: "application/json".to_string(),
                schema: Some(SchemaNode::object()),
            }],
            examples: default_mock_examples(),
            auth: None,
        }
    }

    fn collection(endpoints: Vec<CanonicalEndpoint>) -> CanonicalApiCollection {
        CanonicalApiCollection {
            id: "orders".to_string(),
            name: "Orders".to_string(),
            source: InputSourceKind::OpenApi,
            description: None,
            endpoints,
        }
    }

    #[test]
    fn validate_mock_payload_uses_canonical_schema_validator() {
        let mut schema = SchemaNode::object();
        let mut id = SchemaNode::string();
        id.required = true;
        schema.properties.insert("id".to_string(), id);
        schema.allow_additional_properties = false;

        let result = validate_mock_payload(ValidateMockPayloadArgs {
            schema,
            payload: json!({"extra": true}),
        });

        assert!(!result.valid);
        assert!(
            result
                .errors
                .iter()
                .any(|error| error.contains("$.id: required property missing"))
        );
        assert!(
            result
                .errors
                .iter()
                .any(|error| error.contains("$.extra: additional property not allowed"))
        );
    }

    #[test]
    fn validate_mock_payload_accepts_matching_payloads() {
        let mut schema = SchemaNode::string();
        schema.node_type = SchemaNodeType::Integer;

        let result = validate_mock_payload(ValidateMockPayloadArgs {
            schema,
            payload: json!(42),
        });

        assert!(result.valid);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn diff_collections_reports_endpoint_drift() {
        let previous = collection(vec![
            endpoint(HttpMethod::Get, "/orders", "List orders"),
            endpoint(HttpMethod::Get, "/orders/{id}", "Get order"),
            endpoint(HttpMethod::Delete, "/orders/{id}", "Delete order"),
        ]);
        let next = collection(vec![
            endpoint(HttpMethod::Get, "/orders", "List orders"),
            endpoint(HttpMethod::Get, "/orders/{id}", "Fetch order"),
            endpoint(HttpMethod::Post, "/orders", "Create order"),
        ]);

        let diff = diff_collections(&previous, &next);

        assert_eq!(diff.unchanged, 1);
        assert_eq!(diff.added.len(), 1);
        assert_eq!(diff.added[0].method, "POST");
        assert_eq!(diff.added[0].path, "/orders");
        assert_eq!(diff.changed.len(), 1);
        assert_eq!(diff.changed[0].method, "GET");
        assert_eq!(diff.changed[0].path, "/orders/{id}");
        assert_eq!(diff.changed[0].reasons, vec!["metadata changed"]);
        assert_eq!(diff.removed.len(), 1);
        assert_eq!(diff.removed[0].method, "DELETE");
        assert_eq!(diff.removed[0].path, "/orders/{id}");
    }

    #[test]
    fn diff_collections_ignores_mock_example_changes() {
        let previous = collection(vec![endpoint(HttpMethod::Get, "/orders", "List orders")]);
        let mut next = previous.clone();
        next.endpoints[0].examples[0].payload = json!({"edited": true});

        let diff = diff_collections(&previous, &next);

        assert_eq!(diff.unchanged, 1);
        assert!(diff.added.is_empty());
        assert!(diff.changed.is_empty());
        assert!(diff.removed.is_empty());
    }

    #[test]
    fn diff_collections_explains_contract_change_reasons() {
        let previous_endpoint = endpoint(HttpMethod::Post, "/orders", "Create order");
        let mut next_endpoint = previous_endpoint.clone();
        next_endpoint.parameters.push(CanonicalParameter {
            name: "X-Trace-Id".to_string(),
            location: albert_core::ParameterLocation::Header,
            description: None,
            required: false,
            schema: SchemaNode::string(),
        });
        next_endpoint.request_body = Some(albert_core::CanonicalRequestBody {
            content_type: "application/json".to_string(),
            required: true,
            schema: SchemaNode::object(),
        });
        next_endpoint.responses[0].status_code = "201".to_string();
        next_endpoint.auth = Some(albert_core::AuthRequirement {
            scheme: albert_core::AuthScheme::HttpBearer,
            header_name: "Authorization".to_string(),
            value_prefix: Some("Bearer ".to_string()),
            description: None,
        });

        let diff = diff_collections(
            &collection(vec![previous_endpoint]),
            &collection(vec![next_endpoint]),
        );

        assert_eq!(diff.changed.len(), 1);
        assert_eq!(
            diff.changed[0].reasons,
            vec![
                "parameters changed",
                "request body changed",
                "responses changed",
                "auth changed"
            ]
        );
        assert_eq!(
            diff.changed[0].details,
            vec![
                "parameter added: header X-Trace-Id",
                "request body added: application/json (required)",
                "response added: 201 application/json",
                "response removed: 200 application/json",
                "auth requirement changed"
            ]
        );
    }

    #[test]
    fn new_import_diff_marks_every_endpoint_added() {
        let collection = collection(vec![
            endpoint(HttpMethod::Get, "/orders", "List orders"),
            endpoint(HttpMethod::Post, "/orders", "Create order"),
        ]);

        let diff = ImportDiffSummary::for_new_import(&collection);

        assert_eq!(diff.added.len(), 2);
        assert_eq!(diff.added[0].method, "GET");
        assert_eq!(diff.added[1].method, "POST");
        assert_eq!(diff.unchanged, 0);
        assert!(diff.changed.is_empty());
        assert!(diff.removed.is_empty());
    }

    #[test]
    fn import_api_description_returns_diff_for_reimport() {
        let temp_file = tempfile::NamedTempFile::new().unwrap();
        let db = temp_file.path().to_string_lossy().to_string();
        let first = r#"
openapi: 3.0.3
info:
  title: Orders
  version: 1.0.0
paths:
  /orders:
    get:
      summary: List orders
      responses:
        "200":
          description: OK
  /orders/{id}:
    get:
      summary: Get order
      responses:
        "200":
          description: OK
"#;
        let second = r#"
openapi: 3.0.3
info:
  title: Orders
  version: 1.0.0
paths:
  /orders:
    get:
      summary: List orders
      responses:
        "200":
          description: OK
    post:
      summary: Create order
      responses:
        "201":
          description: Created
  /orders/{id}:
    get:
      summary: Fetch order by id
      responses:
        "200":
          description: OK
"#;

        let initial = import_api_description(
            first.to_string(),
            Some("Orders".to_string()),
            Some(db.clone()),
        )
        .unwrap();
        let updated =
            import_api_description(second.to_string(), Some("Orders".to_string()), Some(db))
                .unwrap();

        assert_eq!(initial.diff.added.len(), 2);
        assert_eq!(updated.diff.added.len(), 1);
        assert_eq!(updated.diff.changed.len(), 1);
        assert_eq!(updated.diff.removed.len(), 0);
        assert_eq!(updated.diff.unchanged, 1);
        assert_eq!(updated.diff.added[0].method, "POST");
        assert_eq!(updated.diff.changed[0].path, "/orders/{id}");
    }
}
