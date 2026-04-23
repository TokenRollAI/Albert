use std::collections::BTreeMap;

use albert_core::{
    CanonicalApiCollection, CanonicalEndpoint, CanonicalParameter, CanonicalRequestBody,
    CanonicalResponse, HttpMethod, InputSourceKind, ParameterLocation, SchemaNode,
    default_mock_examples,
};
use serde_json::Value;
use url::Url;

use crate::{ApiParser, ParseError, ParseSource, schema_from_json_value};

#[derive(Debug, Default)]
pub struct CurlParser;

impl ApiParser for CurlParser {
    fn kind(&self) -> InputSourceKind {
        InputSourceKind::Curl
    }

    fn parse(&self, source: ParseSource) -> Result<CanonicalApiCollection, ParseError> {
        let normalized = source
            .body
            .replace("\\\r\n", " ")
            .replace("\\\n", " ")
            .replace('\n', " ");

        let tokens = shlex::split(&normalized)
            .ok_or_else(|| ParseError::ParseFailed("failed to tokenize cURL input".to_string()))?;

        if tokens.is_empty() || tokens[0] != "curl" {
            return Err(ParseError::InvalidSource(
                "cURL input should begin with the curl command".to_string(),
            ));
        }

        let parsed = parse_curl_tokens(&tokens[1..])?;

        Ok(CanonicalApiCollection {
            id: canonical_id(source.name.as_deref().unwrap_or("imported-curl-request")),
            name: source
                .name
                .unwrap_or_else(|| parsed.path.trim_matches('/').replace('/', "-")),
            source: InputSourceKind::Curl,
            description: Some("Imported from cURL".to_string()),
            endpoints: vec![CanonicalEndpoint {
                operation_id: None,
                method: parsed.method,
                path: parsed.path,
                summary: Some("Imported cURL request".to_string()),
                description: Some("Canonical endpoint generated from cURL input.".to_string()),
                tags: vec!["curl-import".to_string()],
                parameters: parsed.parameters,
                request_body: parsed.request_body,
                responses: vec![CanonicalResponse {
                    status_code: "200".to_string(),
                    description: Some("Default response placeholder for cURL imports.".to_string()),
                    content_type: "application/json".to_string(),
                    schema: None,
                }],
                examples: default_mock_examples(),
            }],
        })
    }
}

struct ParsedCurlRequest {
    method: HttpMethod,
    path: String,
    parameters: Vec<CanonicalParameter>,
    request_body: Option<CanonicalRequestBody>,
}

fn parse_curl_tokens(tokens: &[String]) -> Result<ParsedCurlRequest, ParseError> {
    let mut explicit_method = None;
    let mut headers = BTreeMap::<String, String>::new();
    let mut body = None::<String>;
    let mut url = None::<String>;

    let mut index = 0;
    while index < tokens.len() {
        let token = &tokens[index];
        match token.as_str() {
            "-X" | "--request" => {
                index += 1;
                let value = tokens.get(index).ok_or_else(|| {
                    ParseError::ParseFailed("missing method after -X/--request".to_string())
                })?;
                explicit_method = Some(http_method_from_token(value)?);
            }
            "-H" | "--header" => {
                index += 1;
                let header = tokens.get(index).ok_or_else(|| {
                    ParseError::ParseFailed("missing header value after -H/--header".to_string())
                })?;
                if let Some((name, value)) = parse_header(header) {
                    headers.insert(name, value);
                }
            }
            "-d" | "--data" | "--data-raw" | "--data-binary" | "--data-ascii" => {
                index += 1;
                let value = tokens.get(index).ok_or_else(|| {
                    ParseError::ParseFailed("missing request body after data flag".to_string())
                })?;
                body = Some(value.clone());
            }
            "--url" => {
                index += 1;
                let value = tokens.get(index).ok_or_else(|| {
                    ParseError::ParseFailed("missing URL after --url".to_string())
                })?;
                url = Some(value.clone());
            }
            other if looks_like_url(other) => {
                url = Some(other.to_string());
            }
            _ => {}
        }
        index += 1;
    }

    let url =
        url.ok_or_else(|| ParseError::ParseFailed("cURL input is missing a URL".to_string()))?;
    let parsed_url = Url::parse(&url)
        .or_else(|_| Url::parse(&format!("http://placeholder{url}")))
        .map_err(|error| ParseError::ParseFailed(format!("failed to parse cURL URL: {error}")))?;

    let method = explicit_method.unwrap_or_else(|| {
        if body.is_some() {
            HttpMethod::Post
        } else {
            HttpMethod::Get
        }
    });

    let mut parameters = Vec::new();
    for (name, value) in parsed_url.query_pairs() {
        parameters.push(CanonicalParameter {
            name: name.into_owned(),
            location: ParameterLocation::Query,
            description: None,
            required: false,
            schema: string_schema_with_example(Value::String(value.into_owned())),
        });
    }

    for (name, value) in &headers {
        if is_protocol_header(name) {
            continue;
        }

        parameters.push(CanonicalParameter {
            name: name.clone(),
            location: ParameterLocation::Header,
            description: None,
            required: false,
            schema: string_schema_with_example(Value::String(value.clone())),
        });
    }

    let request_body = body
        .as_ref()
        .map(|body| canonical_request_body(body, headers.get("Content-Type").cloned()))
        .transpose()?;

    Ok(ParsedCurlRequest {
        method,
        path: parsed_url.path().to_string(),
        parameters,
        request_body,
    })
}

fn canonical_request_body(
    body: &str,
    content_type: Option<String>,
) -> Result<CanonicalRequestBody, ParseError> {
    let resolved_content_type = content_type.unwrap_or_else(|| {
        if serde_json::from_str::<Value>(body).is_ok() {
            "application/json".to_string()
        } else {
            "text/plain".to_string()
        }
    });

    let schema = if resolved_content_type.contains("json") {
        let value = serde_json::from_str::<Value>(body).map_err(|error| {
            ParseError::ParseFailed(format!("failed to parse JSON request body: {error}"))
        })?;
        schema_from_json_value(&value)
    } else {
        string_schema_with_example(Value::String(body.to_string()))
    };

    Ok(CanonicalRequestBody {
        content_type: resolved_content_type,
        required: true,
        schema,
    })
}

fn parse_header(header: &str) -> Option<(String, String)> {
    let (name, value) = header.split_once(':')?;
    Some((name.trim().to_string(), value.trim().to_string()))
}

fn is_protocol_header(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "accept" | "authorization" | "content-type"
    )
}

fn looks_like_url(token: &str) -> bool {
    token.starts_with("http://") || token.starts_with("https://") || token.starts_with('/')
}

fn http_method_from_token(token: &str) -> Result<HttpMethod, ParseError> {
    match token.to_ascii_uppercase().as_str() {
        "GET" => Ok(HttpMethod::Get),
        "POST" => Ok(HttpMethod::Post),
        "PUT" => Ok(HttpMethod::Put),
        "PATCH" => Ok(HttpMethod::Patch),
        "DELETE" => Ok(HttpMethod::Delete),
        "OPTIONS" => Ok(HttpMethod::Options),
        "HEAD" => Ok(HttpMethod::Head),
        unsupported => Err(ParseError::ParseFailed(format!(
            "unsupported HTTP method in cURL input: {unsupported}"
        ))),
    }
}

fn string_schema_with_example(example: Value) -> SchemaNode {
    let mut schema = SchemaNode::string();
    schema.example = Some(example);
    schema
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
