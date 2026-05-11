use albert_core::{
    CanonicalApiCollection, CanonicalEndpoint, CanonicalParameter, CanonicalRequestBody,
    CanonicalResponse, HttpMethod, InputSourceKind, ParameterLocation, SchemaNode,
    synthesize_examples,
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

        let mut endpoint = CanonicalEndpoint {
            operation_id: None,
            method: parsed.method,
            path: parsed.path.clone(),
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
            examples: Vec::new(),
            auth: None,
        };
        endpoint.examples = synthesize_examples(&endpoint);
        Ok(CanonicalApiCollection {
            id: canonical_id(source.name.as_deref().unwrap_or("imported-curl-request")),
            name: source
                .name
                .unwrap_or_else(|| parsed.path.trim_matches('/').replace('/', "-")),
            source: InputSourceKind::Curl,
            description: Some("Imported from cURL".to_string()),
            endpoints: vec![endpoint],
        })
    }
}

struct ParsedCurlRequest {
    method: HttpMethod,
    path: String,
    parameters: Vec<CanonicalParameter>,
    request_body: Option<CanonicalRequestBody>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RawBodyKind {
    Text,
    Binary,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct HeaderEntry {
    name: String,
    value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FormPart {
    name: String,
    value: String,
    is_file: bool,
    content_type: Option<String>,
}

fn parse_curl_tokens(tokens: &[String]) -> Result<ParsedCurlRequest, ParseError> {
    let mut explicit_method = None;
    let mut headers = Vec::<HeaderEntry>::new();
    let mut raw_body_parts = Vec::<(String, RawBodyKind)>::new();
    let mut url = None::<String>;
    let mut urlencoded_pairs: Vec<String> = Vec::new();
    let mut form_parts = Vec::<FormPart>::new();

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
                    headers.push(HeaderEntry { name, value });
                }
            }
            "-d" | "--data" | "--data-raw" | "--data-ascii" => {
                index += 1;
                let value = tokens.get(index).ok_or_else(|| {
                    ParseError::ParseFailed("missing request body after data flag".to_string())
                })?;
                raw_body_parts.push((value.clone(), RawBodyKind::Text));
            }
            "--data-binary" => {
                index += 1;
                let value = tokens.get(index).ok_or_else(|| {
                    ParseError::ParseFailed("missing request body after data flag".to_string())
                })?;
                raw_body_parts.push((value.clone(), RawBodyKind::Binary));
            }
            "--data-urlencode" => {
                index += 1;
                let value = tokens.get(index).ok_or_else(|| {
                    ParseError::ParseFailed("missing value after --data-urlencode".to_string())
                })?;
                urlencoded_pairs.push(value.clone());
            }
            "-F" | "--form" => {
                index += 1;
                let value = tokens.get(index).ok_or_else(|| {
                    ParseError::ParseFailed("missing value after -F/--form".to_string())
                })?;
                form_parts.push(parse_form_part(value, false)?);
            }
            "--form-string" => {
                index += 1;
                let value = tokens.get(index).ok_or_else(|| {
                    ParseError::ParseFailed("missing value after --form-string".to_string())
                })?;
                form_parts.push(parse_form_part(value, true)?);
            }
            "-u" | "--user" => {
                index += 1;
                let value = tokens.get(index).ok_or_else(|| {
                    ParseError::ParseFailed("missing credential after -u/--user".to_string())
                })?;
                // RFC 7617 Basic auth. We record the literal `user:pass` string so
                // the UI can surface that auth is expected without materializing
                // a base64 blob in logs/tests.
                if !has_header(&headers, "Authorization") {
                    headers.push(HeaderEntry {
                        name: "Authorization".to_string(),
                        value: format!("Basic {}", value),
                    });
                }
            }
            "-b" | "--cookie" => {
                index += 1;
                let value = tokens.get(index).ok_or_else(|| {
                    ParseError::ParseFailed("missing value after -b/--cookie".to_string())
                })?;
                if !has_header(&headers, "Cookie") {
                    headers.push(HeaderEntry {
                        name: "Cookie".to_string(),
                        value: value.clone(),
                    });
                }
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

    if !form_parts.is_empty() && (!raw_body_parts.is_empty() || !urlencoded_pairs.is_empty()) {
        return Err(ParseError::ParseFailed(
            "multipart form flags cannot be combined with data body flags".to_string(),
        ));
    }

    if !urlencoded_pairs.is_empty() && raw_body_parts.is_empty() {
        let encoded = urlencoded_pairs
            .iter()
            .map(|raw| match raw.find('=') {
                Some(idx) => {
                    let (k, v) = raw.split_at(idx);
                    format!("{}={}", urlencoding_encode(k), urlencoding_encode(&v[1..]))
                }
                None => urlencoding_encode(raw),
            })
            .collect::<Vec<_>>()
            .join("&");
        raw_body_parts.push((encoded, RawBodyKind::Text));
        if !has_header(&headers, "Content-Type") {
            headers.push(HeaderEntry {
                name: "Content-Type".to_string(),
                value: "application/x-www-form-urlencoded".to_string(),
            });
        }
    }

    let url =
        url.ok_or_else(|| ParseError::ParseFailed("cURL input is missing a URL".to_string()))?;
    let parsed_url = Url::parse(&url)
        .or_else(|_| Url::parse(&format!("http://placeholder{url}")))
        .map_err(|error| ParseError::ParseFailed(format!("failed to parse cURL URL: {error}")))?;

    let method = explicit_method.unwrap_or({
        if !raw_body_parts.is_empty() || !form_parts.is_empty() {
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

    for header in &headers {
        if is_protocol_header(&header.name) {
            continue;
        }

        parameters.push(CanonicalParameter {
            name: header.name.clone(),
            location: ParameterLocation::Header,
            description: None,
            required: false,
            schema: string_schema_with_example(Value::String(header.value.clone())),
        });
    }

    let content_type = last_header_value(&headers, "Content-Type");
    let request_body = if !form_parts.is_empty() {
        Some(canonical_multipart_request_body(
            &form_parts,
            content_type.or_else(|| Some("multipart/form-data".to_string())),
        ))
    } else if raw_body_parts.is_empty() {
        None
    } else {
        let body = raw_body_parts
            .iter()
            .map(|(value, _)| value.as_str())
            .collect::<Vec<_>>()
            .join("&");
        let kind = if raw_body_parts
            .iter()
            .any(|(_, kind)| *kind == RawBodyKind::Binary)
        {
            RawBodyKind::Binary
        } else {
            RawBodyKind::Text
        };
        Some(canonical_request_body(&body, content_type, kind)?)
    };

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
    kind: RawBodyKind,
) -> Result<CanonicalRequestBody, ParseError> {
    let resolved_content_type = content_type.unwrap_or_else(|| {
        if kind == RawBodyKind::Binary {
            "application/octet-stream".to_string()
        } else if serde_json::from_str::<Value>(body).is_ok() {
            "application/json".to_string()
        } else {
            "text/plain".to_string()
        }
    });

    let schema = if kind == RawBodyKind::Binary && is_curl_file_reference(body) {
        binary_schema_with_example(body.to_string(), None)
    } else if resolved_content_type.contains("json") {
        let value = serde_json::from_str::<Value>(body).map_err(|error| {
            ParseError::ParseFailed(format!("failed to parse JSON request body: {error}"))
        })?;
        schema_from_json_value(&value)
    } else if kind == RawBodyKind::Binary {
        binary_schema_with_example(body.to_string(), None)
    } else {
        string_schema_with_example(Value::String(body.to_string()))
    };

    Ok(CanonicalRequestBody {
        content_type: resolved_content_type,
        required: true,
        schema,
    })
}

fn canonical_multipart_request_body(
    parts: &[FormPart],
    content_type: Option<String>,
) -> CanonicalRequestBody {
    let mut schema = SchemaNode::object();
    for part in parts {
        let mut child = if part.is_file {
            binary_schema_with_example(part.value.clone(), part.content_type.clone())
        } else {
            string_schema_with_example(Value::String(part.value.clone()))
        };
        child.required = true;
        schema.properties.insert(part.name.clone(), child);
    }

    CanonicalRequestBody {
        content_type: content_type.unwrap_or_else(|| "multipart/form-data".to_string()),
        required: true,
        schema,
    }
}

/// Minimal percent-encoder covering the subset of characters the curl
/// `--data-urlencode` flag typically normalizes. We only escape characters
/// that are unsafe in a form body.
fn urlencoding_encode(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for byte in input.as_bytes() {
        let byte = *byte;
        let is_safe = byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~');
        if is_safe {
            out.push(byte as char);
        } else if byte == b' ' {
            out.push('+');
        } else {
            out.push_str(&format!("%{:02X}", byte));
        }
    }
    out
}

fn parse_form_part(input: &str, force_literal: bool) -> Result<FormPart, ParseError> {
    let (name, raw_value) = input.split_once('=').ok_or_else(|| {
        ParseError::ParseFailed("multipart form values must use name=value syntax".to_string())
    })?;
    let name = name.trim();
    if name.is_empty() {
        return Err(ParseError::ParseFailed(
            "multipart form field name cannot be empty".to_string(),
        ));
    }

    let mut segments = raw_value.split(';');
    let first = segments.next().unwrap_or_default().to_string();
    let content_type = segments
        .filter_map(|segment| segment.strip_prefix("type="))
        .next()
        .map(str::to_string);
    let is_file = !force_literal && is_curl_file_reference(&first);

    Ok(FormPart {
        name: name.to_string(),
        value: first,
        is_file,
        content_type,
    })
}

fn parse_header(header: &str) -> Option<(String, String)> {
    let (name, value) = header.split_once(':')?;
    Some((name.trim().to_string(), value.trim().to_string()))
}

fn has_header(headers: &[HeaderEntry], name: &str) -> bool {
    headers
        .iter()
        .any(|header| header.name.eq_ignore_ascii_case(name))
}

fn last_header_value(headers: &[HeaderEntry], name: &str) -> Option<String> {
    headers
        .iter()
        .rev()
        .find(|header| header.name.eq_ignore_ascii_case(name))
        .map(|header| header.value.clone())
}

fn is_protocol_header(name: &str) -> bool {
    // Content-Type is already expressed on the request body. Other headers —
    // including Authorization, Cookie, Accept — are meaningful to preserve
    // as canonical parameters so downstream consumers (UI, tests) can see
    // them.
    matches!(name.to_ascii_lowercase().as_str(), "content-type")
}

fn looks_like_url(token: &str) -> bool {
    token.starts_with("http://") || token.starts_with("https://") || token.starts_with('/')
}

fn is_curl_file_reference(value: &str) -> bool {
    value.starts_with('@') || value.starts_with('<')
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

fn binary_schema_with_example(example: String, content_type: Option<String>) -> SchemaNode {
    let mut schema = string_schema_with_example(Value::String(example));
    schema.format = Some("binary".to_string());
    schema.description = content_type.map(|value| format!("Binary upload part ({value})"));
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
