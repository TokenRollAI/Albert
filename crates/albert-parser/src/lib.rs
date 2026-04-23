mod curl;
mod openapi;

pub use curl::CurlParser;
pub use openapi::OpenApiParser;

use albert_core::{
    CanonicalApiCollection, CapabilityStatus, DeliveryStage, InputSourceKind, SchemaNode,
    SchemaNodeType,
};
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct ParseSource {
    pub name: Option<String>,
    pub body: String,
}

pub trait ApiParser {
    fn kind(&self) -> InputSourceKind;
    fn parse(&self, source: ParseSource) -> Result<CanonicalApiCollection, ParseError>;
}

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("unsupported input source")]
    UnsupportedInput,
    #[error("invalid source: {0}")]
    InvalidSource(String),
    #[error("parse failed: {0}")]
    ParseFailed(String),
}

pub fn planned_capabilities() -> Vec<CapabilityStatus> {
    vec![
        CapabilityStatus {
            name: "OpenAPI parser".to_string(),
            stage: DeliveryStage::Partial,
            note: "OpenAPI JSON and YAML parsing is implemented for core request and response shapes."
                .to_string(),
        },
        CapabilityStatus {
            name: "cURL parser".to_string(),
            stage: DeliveryStage::Partial,
            note: "Common request flags and JSON request bodies are normalized into canonical endpoints."
                .to_string(),
        },
        CapabilityStatus {
            name: "Canonical schema transform".to_string(),
            stage: DeliveryStage::Partial,
            note: "OpenAPI schemas and JSON payloads are converted into canonical schema nodes."
                .to_string(),
        },
    ]
}

pub fn detect_parser(raw: &str) -> Result<InputSourceKind, ParseError> {
    let trimmed = raw.trim_start();

    if trimmed.starts_with("curl ") {
        return Ok(InputSourceKind::Curl);
    }

    if trimmed.contains("\"openapi\"") || trimmed.contains("openapi:") {
        return Ok(InputSourceKind::OpenApi);
    }

    Err(ParseError::UnsupportedInput)
}

pub fn parse_source(source: ParseSource) -> Result<CanonicalApiCollection, ParseError> {
    match detect_parser(&source.body)? {
        InputSourceKind::OpenApi => OpenApiParser.parse(source),
        InputSourceKind::Curl => CurlParser.parse(source),
    }
}

pub(crate) fn schema_from_json_value(value: &Value) -> SchemaNode {
    match value {
        Value::Object(map) => {
            let mut node = SchemaNode::object();
            for (key, nested_value) in map {
                let mut child = schema_from_json_value(nested_value);
                child.required = true;
                node.properties.insert(key.clone(), child);
            }
            node.example = Some(value.clone());
            node
        }
        Value::Array(items) => {
            let inferred_item = items
                .first()
                .map(schema_from_json_value)
                .unwrap_or_else(SchemaNode::object);

            let mut node = SchemaNode::array(inferred_item);
            node.example = Some(value.clone());
            node
        }
        Value::String(_) => {
            let mut node = SchemaNode::string();
            node.example = Some(value.clone());
            node
        }
        Value::Number(number) if number.is_i64() || number.is_u64() => SchemaNode {
            node_type: SchemaNodeType::Integer,
            description: None,
            required: false,
            nullable: false,
            properties: Default::default(),
            items: None,
            enum_values: Vec::new(),
            example: Some(value.clone()),
        },
        Value::Number(_) => SchemaNode {
            node_type: SchemaNodeType::Number,
            description: None,
            required: false,
            nullable: false,
            properties: Default::default(),
            items: None,
            enum_values: Vec::new(),
            example: Some(value.clone()),
        },
        Value::Bool(_) => SchemaNode {
            node_type: SchemaNodeType::Boolean,
            description: None,
            required: false,
            nullable: false,
            properties: Default::default(),
            items: None,
            enum_values: Vec::new(),
            example: Some(value.clone()),
        },
        Value::Null => SchemaNode {
            node_type: SchemaNodeType::Null,
            description: None,
            required: false,
            nullable: true,
            properties: Default::default(),
            items: None,
            enum_values: Vec::new(),
            example: Some(Value::Null),
        },
    }
}

#[cfg(test)]
mod tests {
    use albert_core::{HttpMethod, InputSourceKind, ParameterLocation, SchemaNodeType};

    use crate::{ParseSource, detect_parser, parse_source};

    #[test]
    fn detects_openapi_json_sources() {
        let input = r#"{"openapi":"3.0.3","info":{"title":"Demo","version":"1.0.0"},"paths":{}}"#;

        assert_eq!(detect_parser(input).unwrap(), InputSourceKind::OpenApi);
    }

    #[test]
    fn detects_openapi_yaml_sources() {
        let input = "openapi: 3.0.3\ninfo:\n  title: Demo\n  version: 1.0.0\npaths: {}";

        assert_eq!(detect_parser(input).unwrap(), InputSourceKind::OpenApi);
    }

    #[test]
    fn parses_openapi_fixture_into_canonical_collection() {
        let collection = parse_source(ParseSource {
            name: Some("fixture".to_string()),
            body: include_str!("../../../fixtures/sample-openapi.json").to_string(),
        })
        .unwrap();

        assert_eq!(collection.source, InputSourceKind::OpenApi);
        assert_eq!(collection.endpoints.len(), 1);

        let endpoint = &collection.endpoints[0];
        assert_eq!(endpoint.method, HttpMethod::Get);
        assert_eq!(endpoint.path, "/api/orders");
        assert_eq!(endpoint.parameters.len(), 1);
        assert_eq!(endpoint.parameters[0].location, ParameterLocation::Query);
        assert_eq!(
            endpoint.parameters[0].schema.node_type,
            SchemaNodeType::String
        );
    }

    #[test]
    fn parses_openapi_components_and_request_body_refs() {
        let source = r#"
openapi: 3.0.3
info:
  title: Orders API
  version: 1.0.0
paths:
  /orders:
    post:
      operationId: createOrder
      requestBody:
        $ref: '#/components/requestBodies/CreateOrder'
      responses:
        "201":
          $ref: '#/components/responses/OrderCreated'
components:
  requestBodies:
    CreateOrder:
      required: true
      content:
        application/json:
          schema:
            $ref: '#/components/schemas/CreateOrderInput'
  responses:
    OrderCreated:
      description: Created
      content:
        application/json:
          schema:
            $ref: '#/components/schemas/Order'
  schemas:
    CreateOrderInput:
      type: object
      required: [customer_id]
      properties:
        customer_id:
          type: string
        note:
          type: string
    Order:
      type: object
      required: [id]
      properties:
        id:
          type: string
        total:
          type: number
"#;

        let collection = parse_source(ParseSource {
            name: None,
            body: source.to_string(),
        })
        .unwrap();

        let endpoint = &collection.endpoints[0];
        let request_body = endpoint.request_body.as_ref().unwrap();

        assert_eq!(request_body.content_type, "application/json");
        assert_eq!(
            request_body.schema.properties["customer_id"].node_type,
            SchemaNodeType::String
        );
        assert!(request_body.schema.properties["customer_id"].required);
        assert_eq!(endpoint.responses[0].status_code, "201");
        assert_eq!(
            endpoint.responses[0].schema.as_ref().unwrap().properties["total"].node_type,
            SchemaNodeType::Number
        );
    }

    #[test]
    fn parses_openapi_path_parameters_as_required() {
        let source = r#"
openapi: 3.0.3
info:
  title: Users API
  version: 1.0.0
paths:
  /users/{id}:
    parameters:
      - name: id
        in: path
        required: true
        schema:
          type: string
    get:
      responses:
        "200":
          description: OK
"#;

        let collection = parse_source(ParseSource {
            name: None,
            body: source.to_string(),
        })
        .unwrap();

        let endpoint = &collection.endpoints[0];
        assert_eq!(endpoint.parameters.len(), 1);
        assert_eq!(endpoint.parameters[0].location, ParameterLocation::Path);
        assert!(endpoint.parameters[0].required);
    }

    #[test]
    fn parses_curl_fixture_into_canonical_collection() {
        let collection = parse_source(ParseSource {
            name: None,
            body: include_str!("../../../fixtures/sample-curl.txt").to_string(),
        })
        .unwrap();

        assert_eq!(collection.source, InputSourceKind::Curl);
        assert_eq!(collection.endpoints.len(), 1);

        let endpoint = &collection.endpoints[0];
        assert_eq!(endpoint.method, HttpMethod::Post);
        assert_eq!(endpoint.path, "/api/orders");
        assert_eq!(
            endpoint.request_body.as_ref().unwrap().schema.properties["customer_id"].node_type,
            SchemaNodeType::String
        );
        assert_eq!(
            endpoint.request_body.as_ref().unwrap().schema.properties["items"].node_type,
            SchemaNodeType::Array
        );
    }

    #[test]
    fn parses_curl_query_and_infers_get_without_body() {
        let collection = parse_source(ParseSource {
            name: None,
            body: r#"curl "https://api.example.com/orders?status=pending&page=2" -H "Accept: application/json""#
                .to_string(),
        })
        .unwrap();

        let endpoint = &collection.endpoints[0];

        assert_eq!(endpoint.method, HttpMethod::Get);
        // 2 query params (status, page) + Accept header
        assert_eq!(endpoint.parameters.len(), 3);
        assert!(
            endpoint
                .parameters
                .iter()
                .any(|p| p.name == "Accept" && p.location == ParameterLocation::Header)
        );
        assert!(endpoint.request_body.is_none());
    }

    #[test]
    fn curl_respects_explicit_method_when_body_exists() {
        let collection = parse_source(ParseSource {
            name: None,
            body: r#"curl -X PUT "https://api.example.com/orders/42" -H "Content-Type: application/json" -d '{"status":"paid"}'"#
                .to_string(),
        })
        .unwrap();

        assert_eq!(collection.endpoints[0].method, HttpMethod::Put);
    }

    #[test]
    fn curl_rejects_invalid_json_body_when_content_type_is_json() {
        let error = parse_source(ParseSource {
            name: None,
            body: r#"curl "https://api.example.com/orders" -H "Content-Type: application/json" -d '{broken-json}'"#
                .to_string(),
        })
        .unwrap_err();

        assert!(matches!(error, crate::ParseError::ParseFailed(_)));
    }

    #[test]
    fn rejects_unknown_input_sources() {
        let error = detect_parser("not a supported format").unwrap_err();

        assert!(matches!(error, crate::ParseError::UnsupportedInput));
    }

    #[test]
    fn parses_array_response_schema() {
        let source = r#"
openapi: 3.0.3
info: { title: users, version: 1.0.0 }
paths:
  /users:
    get:
      responses:
        "200":
          description: list
          content:
            application/json:
              schema:
                type: array
                items:
                  type: object
                  required: [id]
                  properties:
                    id: { type: string }
                    name: { type: string }
"#;
        let collection = parse_source(ParseSource {
            name: None,
            body: source.to_string(),
        })
        .unwrap();
        let endpoint = &collection.endpoints[0];
        let response = &endpoint.responses[0];
        let schema = response.schema.as_ref().unwrap();
        assert_eq!(schema.node_type, SchemaNodeType::Array);
        let item = schema.items.as_ref().unwrap();
        assert_eq!(item.node_type, SchemaNodeType::Object);
        assert!(item.properties.contains_key("id"));
        // synthesized example should be an array of one object
        let success = endpoint
            .examples
            .iter()
            .find(|e| matches!(e.kind, albert_core::MockExampleKind::Success))
            .unwrap();
        let arr = success.payload.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert!(arr[0].get("id").is_some());
    }

    #[test]
    fn parses_all_of_merges_properties() {
        let source = r#"
openapi: 3.0.3
info: { title: api, version: 1.0.0 }
paths:
  /thing:
    get:
      responses:
        "200":
          description: ok
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/Thing'
components:
  schemas:
    Base:
      type: object
      required: [id]
      properties:
        id: { type: string }
    Extra:
      type: object
      properties:
        color: { type: string }
    Thing:
      allOf:
        - $ref: '#/components/schemas/Base'
        - $ref: '#/components/schemas/Extra'
"#;
        let collection = parse_source(ParseSource {
            name: None,
            body: source.to_string(),
        })
        .unwrap();
        let endpoint = &collection.endpoints[0];
        let schema = endpoint.responses[0].schema.as_ref().unwrap();
        assert_eq!(schema.node_type, SchemaNodeType::Object);
        // allOf merge should fold both ids and colors into the same object
        assert!(schema.properties.contains_key("id"));
        assert!(schema.properties.contains_key("color"));
    }

    #[test]
    fn parses_parameter_ref_from_components() {
        let source = r#"
openapi: 3.0.3
info: { title: api, version: 1.0.0 }
paths:
  /items:
    get:
      parameters:
        - $ref: '#/components/parameters/PageSize'
      responses:
        "200": { description: ok }
components:
  parameters:
    PageSize:
      name: page_size
      in: query
      required: true
      schema:
        type: integer
"#;
        let collection = parse_source(ParseSource {
            name: None,
            body: source.to_string(),
        })
        .unwrap();
        let endpoint = &collection.endpoints[0];
        assert_eq!(endpoint.parameters.len(), 1);
        let param = &endpoint.parameters[0];
        assert_eq!(param.name, "page_size");
        assert!(param.required);
        assert_eq!(param.schema.node_type, SchemaNodeType::Integer);
    }

    #[test]
    fn curl_parses_basic_auth_and_cookie() {
        let collection = parse_source(ParseSource {
            name: None,
            body: r#"curl -u alice:secret -b "SESSION=xyz" "https://api.example.com/secure""#
                .to_string(),
        })
        .unwrap();
        let endpoint = &collection.endpoints[0];
        let auth = endpoint
            .parameters
            .iter()
            .find(|p| p.name == "Authorization")
            .expect("authorization header parameter");
        assert_eq!(auth.location, ParameterLocation::Header);
        let example = auth
            .schema
            .example
            .as_ref()
            .and_then(|v| v.as_str())
            .unwrap();
        assert!(example.starts_with("Basic alice:secret"));

        let cookie = endpoint
            .parameters
            .iter()
            .find(|p| p.name == "Cookie")
            .expect("cookie header parameter");
        assert_eq!(
            cookie.schema.example.as_ref().and_then(|v| v.as_str()),
            Some("SESSION=xyz")
        );
    }

    #[test]
    fn curl_expands_data_urlencoded_into_form_body() {
        let collection = parse_source(ParseSource {
            name: None,
            body: r#"curl https://api.example.com/login --data-urlencode "user=alice" --data-urlencode "pass=p@ss w0rd""#.to_string(),
        })
        .unwrap();
        let endpoint = &collection.endpoints[0];
        assert_eq!(endpoint.method, HttpMethod::Post);
        let body = endpoint.request_body.as_ref().expect("request body");
        assert_eq!(body.content_type, "application/x-www-form-urlencoded");
        let example = body
            .schema
            .example
            .as_ref()
            .and_then(|v| v.as_str())
            .unwrap();
        // percent-encoded + form-joined
        assert!(example.contains("user=alice"));
        assert!(example.contains("pass=p%40ss+w0rd"));
    }

    #[test]
    fn synthesizes_enum_string_example() {
        let source = r#"
openapi: 3.0.3
info: { title: api, version: 1.0.0 }
paths:
  /status:
    get:
      responses:
        "200":
          description: ok
          content:
            application/json:
              schema:
                type: object
                required: [state]
                properties:
                  state:
                    type: string
                    enum: [active, archived, draft]
"#;
        let collection = parse_source(ParseSource {
            name: None,
            body: source.to_string(),
        })
        .unwrap();
        let endpoint = &collection.endpoints[0];
        let success = endpoint
            .examples
            .iter()
            .find(|e| matches!(e.kind, albert_core::MockExampleKind::Success))
            .unwrap();
        assert_eq!(success.payload["state"], "active");
    }
}
