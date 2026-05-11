//! Integration test for `OpenAiChatAdapter` against a hand-rolled TCP server.
//!
//! We stand up a minimal HTTP/1.1 responder on an ephemeral port, point the
//! adapter at it, and assert that the canned chat-completions JSON is decoded
//! end-to-end into a `MockExample`.

use albert_core::{
    CanonicalEndpoint, CanonicalResponse, HttpMethod, MockExampleKind, ProviderApiType,
    ProviderConfig, ProviderReasoningEffort, SchemaNode, SchemaNodeType,
};
use albert_openai::{GenerationContext, GenerationIntent, OpenAiChatAdapter};
use serde_json::json;
use std::collections::BTreeMap;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::oneshot;

const CHAT_RESPONSE_BODY: &str = r#"{
  "id": "cmpl-test",
  "object": "chat.completion",
  "choices": [
    {
      "index": 0,
      "message": {
        "role": "assistant",
        "content": "```json\n{\"name\": \"Ada\", \"active\": true}\n```"
      },
      "finish_reason": "stop"
    }
  ]
}"#;

const RESPONSES_RESPONSE_BODY: &str = r#"{
  "id": "resp_test",
  "object": "response",
  "output_text": "{\"name\": \"Ada\", \"active\": true}",
  "output": []
}"#;

fn endpoint() -> CanonicalEndpoint {
    let mut properties = BTreeMap::new();
    let mut name = SchemaNode::string();
    name.required = true;
    properties.insert("name".to_string(), name);
    let schema = SchemaNode {
        node_type: SchemaNodeType::Object,
        required: true,
        properties,
        ..SchemaNode::object()
    };
    CanonicalEndpoint {
        operation_id: Some("createUser".into()),
        method: HttpMethod::Post,
        path: "/users".into(),
        summary: Some("Create a user".into()),
        description: None,
        tags: vec!["users".into()],
        parameters: Vec::new(),
        request_body: None,
        responses: vec![CanonicalResponse {
            status_code: "201".into(),
            description: Some("Created".into()),
            content_type: "application/json".into(),
            schema: Some(schema),
        }],
        examples: Vec::new(),
        auth: None,
    }
}

/// Minimal HTTP/1.1 responder that answers the first request with a fixed body.
/// Returns (bound address, shutdown handle).
async fn spawn_stub(
    response_status: &'static str,
    response_body: &'static str,
) -> (String, oneshot::Sender<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    let (tx, rx) = oneshot::channel::<()>();

    tokio::spawn(async move {
        tokio::select! {
            _ = rx => {}
            Ok((mut socket, _)) = listener.accept() => {
                // Read until we've seen the end of headers (don't need to parse)
                let mut buf = [0u8; 8192];
                let mut accumulated = Vec::new();
                loop {
                    match socket.read(&mut buf).await {
                        Ok(0) => break,
                        Ok(n) => {
                            accumulated.extend_from_slice(&buf[..n]);
                            if accumulated.windows(4).any(|w| w == b"\r\n\r\n") {
                                // Try to consume any content-length body to avoid a client abort.
                                let header_end =
                                    accumulated.windows(4).position(|w| w == b"\r\n\r\n").unwrap_or(0)
                                        + 4;
                                let head = std::str::from_utf8(&accumulated[..header_end]).unwrap_or("");
                                let mut body_len = 0usize;
                                for line in head.lines() {
                                    if let Some(rest) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                                        body_len = rest.trim().parse::<usize>().unwrap_or(0);
                                    }
                                }
                                let already = accumulated.len().saturating_sub(header_end);
                                let remaining = body_len.saturating_sub(already);
                                if remaining > 0 {
                                    let mut remaining_buf = vec![0u8; remaining];
                                    let _ = socket.read_exact(&mut remaining_buf).await;
                                }
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }

                let response = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {len}\r\nConnection: close\r\n\r\n{body}",
                    status = response_status,
                    len = response_body.len(),
                    body = response_body
                );
                let _ = socket.write_all(response.as_bytes()).await;
                let _ = socket.flush().await;
                let _ = socket.shutdown().await;
            }
        }
    });

    (addr, tx)
}

/// Minimal HTTP/1.1 responder that answers two requests in sequence. Used to
/// lock in the schema-repair retry path without pulling in a test server crate.
async fn spawn_sequence_stub(
    responses: Vec<(&'static str, &'static str)>,
) -> (String, oneshot::Sender<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    let (tx, mut rx) = oneshot::channel::<()>();

    tokio::spawn(async move {
        for (response_status, response_body) in responses {
            tokio::select! {
                _ = &mut rx => break,
                Ok((mut socket, _)) = listener.accept() => {
                    let mut buf = [0u8; 8192];
                    let mut accumulated = Vec::new();
                    loop {
                        match socket.read(&mut buf).await {
                            Ok(0) => break,
                            Ok(n) => {
                                accumulated.extend_from_slice(&buf[..n]);
                                if accumulated.windows(4).any(|w| w == b"\r\n\r\n") {
                                    let header_end =
                                        accumulated.windows(4).position(|w| w == b"\r\n\r\n").unwrap_or(0)
                                            + 4;
                                    let head = std::str::from_utf8(&accumulated[..header_end]).unwrap_or("");
                                    let mut body_len = 0usize;
                                    for line in head.lines() {
                                        if let Some(rest) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                                            body_len = rest.trim().parse::<usize>().unwrap_or(0);
                                        }
                                    }
                                    let already = accumulated.len().saturating_sub(header_end);
                                    let remaining = body_len.saturating_sub(already);
                                    if remaining > 0 {
                                        let mut remaining_buf = vec![0u8; remaining];
                                        let _ = socket.read_exact(&mut remaining_buf).await;
                                    }
                                    break;
                                }
                            }
                            Err(_) => break,
                        }
                    }

                    let response = format!(
                        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {len}\r\nConnection: close\r\n\r\n{body}",
                        status = response_status,
                        len = response_body.len(),
                        body = response_body
                    );
                    let _ = socket.write_all(response.as_bytes()).await;
                    let _ = socket.flush().await;
                    let _ = socket.shutdown().await;
                }
            }
        }
    });

    (addr, tx)
}

async fn spawn_capture_stub(
    response_body: &'static str,
) -> (
    String,
    oneshot::Sender<()>,
    tokio::sync::oneshot::Receiver<String>,
) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    let (stop_tx, stop_rx) = oneshot::channel::<()>();
    let (request_tx, request_rx) = oneshot::channel::<String>();

    tokio::spawn(async move {
        tokio::select! {
            _ = stop_rx => {}
            Ok((mut socket, _)) = listener.accept() => {
                let mut buf = [0u8; 8192];
                let mut accumulated = Vec::new();
                loop {
                    match socket.read(&mut buf).await {
                        Ok(0) => break,
                        Ok(n) => {
                            accumulated.extend_from_slice(&buf[..n]);
                            if accumulated.windows(4).any(|w| w == b"\r\n\r\n") {
                                let header_end =
                                    accumulated.windows(4).position(|w| w == b"\r\n\r\n").unwrap_or(0)
                                        + 4;
                                let head = std::str::from_utf8(&accumulated[..header_end]).unwrap_or("");
                                let mut body_len = 0usize;
                                for line in head.lines() {
                                    if let Some(rest) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                                        body_len = rest.trim().parse::<usize>().unwrap_or(0);
                                    }
                                }
                                let already = accumulated.len().saturating_sub(header_end);
                                let remaining = body_len.saturating_sub(already);
                                if remaining > 0 {
                                    let mut remaining_buf = vec![0u8; remaining];
                                    if socket.read_exact(&mut remaining_buf).await.is_ok() {
                                        accumulated.extend_from_slice(&remaining_buf);
                                    }
                                }
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
                let request = String::from_utf8_lossy(&accumulated).to_string();
                let _ = request_tx.send(request);
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {len}\r\nConnection: close\r\n\r\n{body}",
                    len = response_body.len(),
                    body = response_body
                );
                let _ = socket.write_all(response.as_bytes()).await;
                let _ = socket.flush().await;
                let _ = socket.shutdown().await;
            }
        }
    });

    (addr, stop_tx, request_rx)
}

#[tokio::test]
async fn adapter_decodes_chat_completion_content() {
    let (addr, _stop) = spawn_stub("200 OK", CHAT_RESPONSE_BODY).await;
    let config = ProviderConfig {
        provider_name: "stub".into(),
        environment: None,
        base_url: format!("http://{addr}"),
        model: "stub-1".into(),
        api_key_env: "ALBERT_STUB_KEY".into(),
        api_type: ProviderApiType::OpenAiCompatible,
        azure_deployment: None,
        azure_api_version: None,
        temperature: None,
        max_output_tokens: None,
        reasoning_effort: None,
        schema_repair_attempts: None,
    };
    let adapter = OpenAiChatAdapter::new(config).with_api_key("test-key");
    let example = adapter
        .generate_mock_example(&endpoint(), GenerationIntent::Success)
        .await
        .expect("generate");
    assert_eq!(example.kind, MockExampleKind::Success);
    assert_eq!(example.payload["name"], "Ada");
    assert_eq!(example.payload["active"], true);
}

#[tokio::test]
async fn azure_adapter_uses_deployment_url_api_key_header_and_omits_model() {
    let (addr, _stop, request_rx) = spawn_capture_stub(CHAT_RESPONSE_BODY).await;
    let config = ProviderConfig {
        provider_name: "azure".into(),
        environment: None,
        base_url: format!("http://{addr}"),
        model: "fallback-deployment".into(),
        api_key_env: "ALBERT_STUB_KEY".into(),
        api_type: ProviderApiType::AzureOpenAi,
        azure_deployment: Some("orders-deployment".into()),
        azure_api_version: Some("2024-10-21".into()),
        temperature: Some(0.25),
        max_output_tokens: Some(1234),
        reasoning_effort: Some(ProviderReasoningEffort::High),
        schema_repair_attempts: None,
    };
    let adapter = OpenAiChatAdapter::new(config).with_api_key("azure-key");
    let example = adapter
        .generate_mock_example(&endpoint(), GenerationIntent::Success)
        .await
        .expect("generate");

    assert_eq!(example.payload["name"], "Ada");
    let request = request_rx.await.expect("captured request");
    assert!(request.starts_with(
        "POST /openai/deployments/orders-deployment/chat/completions?api-version=2024-10-21 HTTP/1.1"
    ));
    assert!(request.contains("\r\napi-key: azure-key\r\n"));
    assert!(!request.contains("\r\nAuthorization: Bearer"));
    let body = request.split("\r\n\r\n").nth(1).unwrap_or("");
    let body_json: serde_json::Value = serde_json::from_str(body).expect("request body json");
    assert!(body_json.get("model").is_none());
    assert_eq!(body_json["response_format"]["type"], "json_object");
    assert_eq!(body_json["temperature"], json!(0.25));
    assert_eq!(body_json["max_tokens"], json!(1234));
    assert!(body_json.get("reasoning").is_none());
}

#[tokio::test]
async fn responses_adapter_uses_v1_responses_and_extracts_output_text() {
    let (addr, _stop, request_rx) = spawn_capture_stub(RESPONSES_RESPONSE_BODY).await;
    let config = ProviderConfig {
        provider_name: "openai-responses".into(),
        environment: None,
        base_url: format!("http://{addr}/v1"),
        model: "stub-1".into(),
        api_key_env: "ALBERT_STUB_KEY".into(),
        api_type: ProviderApiType::OpenAiResponses,
        azure_deployment: None,
        azure_api_version: None,
        temperature: Some(1.25),
        max_output_tokens: Some(4096),
        reasoning_effort: Some(ProviderReasoningEffort::Low),
        schema_repair_attempts: None,
    };
    let adapter = OpenAiChatAdapter::new(config).with_api_key("test-key");
    let example = adapter
        .generate_mock_example(&endpoint(), GenerationIntent::Success)
        .await
        .expect("generate");

    assert_eq!(example.kind, MockExampleKind::Success);
    assert_eq!(example.payload["name"], "Ada");
    assert_eq!(example.payload["active"], true);

    let request = request_rx.await.expect("captured request");
    assert!(request.starts_with("POST /v1/responses HTTP/1.1"));
    assert!(request.contains("\r\nauthorization: Bearer test-key\r\n"));
    let body = request.split("\r\n\r\n").nth(1).unwrap_or("");
    let body_json: serde_json::Value = serde_json::from_str(body).expect("request body json");
    assert_eq!(body_json["model"], "stub-1");
    assert!(body_json.get("instructions").is_some());
    assert!(body_json.get("input").is_some());
    assert_eq!(body_json["text"]["format"]["type"], "json_object");
    assert_eq!(body_json["temperature"], json!(1.25));
    assert_eq!(body_json["max_output_tokens"], json!(4096));
    assert_eq!(body_json["reasoning"]["effort"], json!("low"));
    assert!(body_json.get("messages").is_none());
}

#[tokio::test]
async fn azure_responses_adapter_uses_openai_v1_responses_with_api_key_header() {
    let (addr, _stop, request_rx) = spawn_capture_stub(RESPONSES_RESPONSE_BODY).await;
    let config = ProviderConfig {
        provider_name: "azure-responses".into(),
        environment: None,
        base_url: format!("http://{addr}"),
        model: "fallback-deployment".into(),
        api_key_env: "AZURE_OPENAI_API_KEY".into(),
        api_type: ProviderApiType::AzureOpenAiResponses,
        azure_deployment: Some("orders-responses-deployment".into()),
        azure_api_version: None,
        temperature: Some(0.25),
        max_output_tokens: Some(2048),
        reasoning_effort: Some(ProviderReasoningEffort::High),
        schema_repair_attempts: None,
    };
    let adapter = OpenAiChatAdapter::new(config).with_api_key("azure-key");
    let example = adapter
        .generate_mock_example(&endpoint(), GenerationIntent::Success)
        .await
        .expect("generate");

    assert_eq!(example.kind, MockExampleKind::Success);
    assert_eq!(example.payload["name"], "Ada");
    assert_eq!(example.payload["active"], true);

    let request = request_rx.await.expect("captured request");
    assert!(request.starts_with("POST /openai/v1/responses HTTP/1.1"));
    assert!(request.contains("\r\napi-key: azure-key\r\n"));
    assert!(!request.contains("\r\nAuthorization: Bearer"));
    let body = request.split("\r\n\r\n").nth(1).unwrap_or("");
    let body_json: serde_json::Value = serde_json::from_str(body).expect("request body json");
    assert_eq!(body_json["model"], "orders-responses-deployment");
    assert!(body_json.get("instructions").is_some());
    assert!(body_json.get("input").is_some());
    assert_eq!(body_json["text"]["format"]["type"], "json_object");
    assert_eq!(body_json["temperature"], json!(0.25));
    assert_eq!(body_json["max_output_tokens"], json!(2048));
    assert_eq!(body_json["reasoning"]["effort"], json!("high"));
    assert!(body_json.get("messages").is_none());
}

#[tokio::test]
async fn adapter_includes_generation_context_in_provider_prompt() {
    let (addr, _stop, request_rx) = spawn_capture_stub(CHAT_RESPONSE_BODY).await;
    let config = ProviderConfig {
        provider_name: "stub".into(),
        environment: None,
        base_url: format!("http://{addr}"),
        model: "stub-1".into(),
        api_key_env: "ALBERT_STUB_KEY".into(),
        api_type: ProviderApiType::OpenAiCompatible,
        azure_deployment: None,
        azure_api_version: None,
        temperature: None,
        max_output_tokens: None,
        reasoning_effort: None,
        schema_repair_attempts: None,
    };
    let adapter = OpenAiChatAdapter::new(config).with_api_key("test-key");
    let context = GenerationContext {
        request_snapshot: Some(json!({
            "query": "status=paid",
            "headers": {"x-trace": "abc"},
            "body": null
        })),
        response_snapshot: Some(json!({
            "status": 200,
            "body": {"name": "Ada"}
        })),
        note: Some("cached fingerprint abc123".to_string()),
    };
    let example = adapter
        .generate_mock_example_with_context(&endpoint(), GenerationIntent::Success, Some(&context))
        .await
        .expect("generate");

    assert_eq!(example.kind, MockExampleKind::Success);
    let request = request_rx.await.expect("captured request");
    let body = request.split("\r\n\r\n").nth(1).unwrap_or("");
    assert!(body.contains("Request context"));
    assert!(body.contains("status=paid"));
    assert!(body.contains("cached fingerprint abc123"));
}

#[tokio::test]
async fn chat_adapter_sends_generation_controls() {
    let (addr, _stop, request_rx) = spawn_capture_stub(CHAT_RESPONSE_BODY).await;
    let config = ProviderConfig {
        provider_name: "stub".into(),
        environment: None,
        base_url: format!("http://{addr}"),
        model: "stub-1".into(),
        api_key_env: "ALBERT_STUB_KEY".into(),
        api_type: ProviderApiType::OpenAiCompatible,
        azure_deployment: None,
        azure_api_version: None,
        temperature: Some(0.5),
        max_output_tokens: Some(1234),
        reasoning_effort: Some(ProviderReasoningEffort::Medium),
        schema_repair_attempts: None,
    };
    let adapter = OpenAiChatAdapter::new(config).with_api_key("test-key");
    let example = adapter
        .generate_mock_example(&endpoint(), GenerationIntent::Success)
        .await
        .expect("generate");

    assert_eq!(example.kind, MockExampleKind::Success);
    let request = request_rx.await.expect("captured request");
    assert!(request.starts_with("POST /v1/chat/completions HTTP/1.1"));
    let body = request.split("\r\n\r\n").nth(1).unwrap_or("");
    let body_json: serde_json::Value = serde_json::from_str(body).expect("request body json");
    assert_eq!(body_json["model"], "stub-1");
    assert_eq!(body_json["temperature"], json!(0.5));
    assert_eq!(body_json["max_tokens"], json!(1234));
    assert!(body_json.get("reasoning").is_none());
}

#[tokio::test]
async fn adapter_surfaces_non_success_status() {
    let (addr, _stop) = spawn_stub("429 Too Many Requests", r#"{"error":"rate_limited"}"#).await;
    let config = ProviderConfig {
        provider_name: "stub".into(),
        environment: None,
        base_url: format!("http://{addr}"),
        model: "stub-1".into(),
        api_key_env: "ALBERT_STUB_KEY".into(),
        api_type: ProviderApiType::OpenAiCompatible,
        azure_deployment: None,
        azure_api_version: None,
        temperature: None,
        max_output_tokens: None,
        reasoning_effort: None,
        schema_repair_attempts: None,
    };
    let adapter = OpenAiChatAdapter::new(config).with_api_key("test-key");
    let err = adapter
        .generate_mock_example(&endpoint(), GenerationIntent::Success)
        .await
        .expect_err("error");
    assert!(err.to_string().contains("429"));
    assert!(err.to_string().contains("rate_limited"));
}

#[tokio::test]
async fn adapter_repairs_payload_after_schema_validation_failure() {
    const BAD_RESPONSE: &str = r#"{
      "id": "cmpl-test-bad",
      "object": "chat.completion",
      "choices": [
        {
          "index": 0,
          "message": {
            "role": "assistant",
            "content": "{\"active\": true}"
          },
          "finish_reason": "stop"
        }
      ]
    }"#;
    const REPAIRED_RESPONSE: &str = r#"{
      "id": "cmpl-test-fixed",
      "object": "chat.completion",
      "choices": [
        {
          "index": 0,
          "message": {
            "role": "assistant",
            "content": "{\"name\": \"Ada\"}"
          },
          "finish_reason": "stop"
        }
      ]
    }"#;
    let (addr, _stop) = spawn_sequence_stub(vec![
        ("200 OK", BAD_RESPONSE),
        ("200 OK", REPAIRED_RESPONSE),
    ])
    .await;
    let config = ProviderConfig {
        provider_name: "stub".into(),
        environment: None,
        base_url: format!("http://{addr}"),
        model: "stub-1".into(),
        api_key_env: "ALBERT_STUB_KEY".into(),
        api_type: ProviderApiType::OpenAiCompatible,
        azure_deployment: None,
        azure_api_version: None,
        temperature: None,
        max_output_tokens: None,
        reasoning_effort: None,
        schema_repair_attempts: None,
    };
    let adapter = OpenAiChatAdapter::new(config).with_api_key("test-key");
    let example = adapter
        .generate_mock_example(&endpoint(), GenerationIntent::Success)
        .await
        .expect("generate");

    assert_eq!(example.kind, MockExampleKind::Success);
    assert_eq!(example.payload["name"], "Ada");
    assert_eq!(
        example.note.as_deref(),
        Some(
            "Generated by OpenAI adapter (success). Repaired after 1 validation retry attempt(s)."
        )
    );
}

#[tokio::test]
async fn adapter_repairs_payload_after_multiple_schema_validation_failures() {
    const BAD_RESPONSE: &str = r#"{
      "id": "cmpl-test-bad",
      "object": "chat.completion",
      "choices": [
        {
          "index": 0,
          "message": {
            "role": "assistant",
            "content": "{\"active\": true}"
          },
          "finish_reason": "stop"
        }
      ]
    }"#;
    const STILL_BAD_RESPONSE: &str = r#"{
      "id": "cmpl-test-still-bad",
      "object": "chat.completion",
      "choices": [
        {
          "index": 0,
          "message": {
            "role": "assistant",
            "content": "{\"name\": 42}"
          },
          "finish_reason": "stop"
        }
      ]
    }"#;
    const REPAIRED_RESPONSE: &str = r#"{
      "id": "cmpl-test-fixed",
      "object": "chat.completion",
      "choices": [
        {
          "index": 0,
          "message": {
            "role": "assistant",
            "content": "{\"name\": \"Ada\"}"
          },
          "finish_reason": "stop"
        }
      ]
    }"#;
    let (addr, _stop) = spawn_sequence_stub(vec![
        ("200 OK", BAD_RESPONSE),
        ("200 OK", STILL_BAD_RESPONSE),
        ("200 OK", REPAIRED_RESPONSE),
    ])
    .await;
    let config = ProviderConfig {
        provider_name: "stub".into(),
        environment: None,
        base_url: format!("http://{addr}"),
        model: "stub-1".into(),
        api_key_env: "ALBERT_STUB_KEY".into(),
        api_type: ProviderApiType::OpenAiCompatible,
        azure_deployment: None,
        azure_api_version: None,
        temperature: None,
        max_output_tokens: None,
        reasoning_effort: None,
        schema_repair_attempts: None,
    };
    let adapter = OpenAiChatAdapter::new(config).with_api_key("test-key");
    let example = adapter
        .generate_mock_example(&endpoint(), GenerationIntent::Success)
        .await
        .expect("generate");

    assert_eq!(example.kind, MockExampleKind::Success);
    assert_eq!(example.payload["name"], "Ada");
    assert_eq!(
        example.note.as_deref(),
        Some(
            "Generated by OpenAI adapter (success). Repaired after 2 validation retry attempt(s)."
        )
    );
}

#[tokio::test]
async fn adapter_respects_disabled_schema_repair_attempts() {
    const BAD_RESPONSE: &str = r#"{
      "id": "cmpl-test-bad",
      "object": "chat.completion",
      "choices": [
        {
          "index": 0,
          "message": {
            "role": "assistant",
            "content": "{\"active\": true}"
          },
          "finish_reason": "stop"
        }
      ]
    }"#;
    const REPAIRED_RESPONSE: &str = r#"{
      "id": "cmpl-test-fixed",
      "object": "chat.completion",
      "choices": [
        {
          "index": 0,
          "message": {
            "role": "assistant",
            "content": "{\"name\": \"Ada\"}"
          },
          "finish_reason": "stop"
        }
      ]
    }"#;
    let (addr, _stop) = spawn_sequence_stub(vec![
        ("200 OK", BAD_RESPONSE),
        ("200 OK", REPAIRED_RESPONSE),
    ])
    .await;
    let config = ProviderConfig {
        provider_name: "stub".into(),
        environment: None,
        base_url: format!("http://{addr}"),
        model: "stub-1".into(),
        api_key_env: "ALBERT_STUB_KEY".into(),
        api_type: ProviderApiType::OpenAiCompatible,
        azure_deployment: None,
        azure_api_version: None,
        temperature: None,
        max_output_tokens: None,
        reasoning_effort: None,
        schema_repair_attempts: Some(0),
    };
    let adapter = OpenAiChatAdapter::new(config).with_api_key("test-key");
    let example = adapter
        .generate_mock_example(&endpoint(), GenerationIntent::Success)
        .await
        .expect("generate");

    assert_eq!(example.kind, MockExampleKind::Success);
    assert_eq!(example.payload["active"], true);
    assert!(
        example
            .note
            .as_deref()
            .unwrap_or_default()
            .contains("repair retries are disabled")
    );
}

#[tokio::test]
async fn adapter_respects_configured_schema_repair_attempt_limit() {
    const BAD_RESPONSE: &str = r#"{
      "id": "cmpl-test-bad",
      "object": "chat.completion",
      "choices": [
        {
          "index": 0,
          "message": {
            "role": "assistant",
            "content": "{\"active\": true}"
          },
          "finish_reason": "stop"
        }
      ]
    }"#;
    const STILL_BAD_RESPONSE: &str = r#"{
      "id": "cmpl-test-still-bad",
      "object": "chat.completion",
      "choices": [
        {
          "index": 0,
          "message": {
            "role": "assistant",
            "content": "{\"name\": 42}"
          },
          "finish_reason": "stop"
        }
      ]
    }"#;
    const REPAIRED_RESPONSE: &str = r#"{
      "id": "cmpl-test-fixed",
      "object": "chat.completion",
      "choices": [
        {
          "index": 0,
          "message": {
            "role": "assistant",
            "content": "{\"name\": \"Ada\"}"
          },
          "finish_reason": "stop"
        }
      ]
    }"#;
    let (addr, _stop) = spawn_sequence_stub(vec![
        ("200 OK", BAD_RESPONSE),
        ("200 OK", STILL_BAD_RESPONSE),
        ("200 OK", REPAIRED_RESPONSE),
    ])
    .await;
    let config = ProviderConfig {
        provider_name: "stub".into(),
        environment: None,
        base_url: format!("http://{addr}"),
        model: "stub-1".into(),
        api_key_env: "ALBERT_STUB_KEY".into(),
        api_type: ProviderApiType::OpenAiCompatible,
        azure_deployment: None,
        azure_api_version: None,
        temperature: None,
        max_output_tokens: None,
        reasoning_effort: None,
        schema_repair_attempts: Some(1),
    };
    let adapter = OpenAiChatAdapter::new(config).with_api_key("test-key");
    let example = adapter
        .generate_mock_example(&endpoint(), GenerationIntent::Success)
        .await
        .expect("generate");

    assert_eq!(example.kind, MockExampleKind::Success);
    assert_eq!(example.payload["name"], 42);
    assert!(
        example
            .note
            .as_deref()
            .unwrap_or_default()
            .contains("still failing after 1 repair attempt(s)")
    );
}
