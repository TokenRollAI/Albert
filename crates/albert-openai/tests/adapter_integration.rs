//! Integration test for `OpenAiChatAdapter` against a hand-rolled TCP server.
//!
//! We stand up a minimal HTTP/1.1 responder on an ephemeral port, point the
//! adapter at it, and assert that the canned chat-completions JSON is decoded
//! end-to-end into a `MockExample`.

use albert_core::{
    CanonicalEndpoint, CanonicalResponse, HttpMethod, MockExampleKind, ProviderConfig, SchemaNode,
    SchemaNodeType,
};
use albert_openai::{GenerationIntent, OpenAiChatAdapter};
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

fn endpoint() -> CanonicalEndpoint {
    let mut properties = BTreeMap::new();
    let mut name = SchemaNode::string();
    name.required = true;
    properties.insert("name".to_string(), name);
    let schema = SchemaNode {
        node_type: SchemaNodeType::Object,
        description: None,
        required: true,
        nullable: false,
        properties,
        items: None,
        enum_values: Vec::new(),
        example: None,
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

#[tokio::test]
async fn adapter_decodes_chat_completion_content() {
    let (addr, _stop) = spawn_stub("200 OK", CHAT_RESPONSE_BODY).await;
    let config = ProviderConfig {
        provider_name: "stub".into(),
        base_url: format!("http://{addr}"),
        model: "stub-1".into(),
        api_key_env: "ALBERT_STUB_KEY".into(),
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
async fn adapter_surfaces_non_success_status() {
    let (addr, _stop) = spawn_stub("429 Too Many Requests", r#"{"error":"rate_limited"}"#).await;
    let config = ProviderConfig {
        provider_name: "stub".into(),
        base_url: format!("http://{addr}"),
        model: "stub-1".into(),
        api_key_env: "ALBERT_STUB_KEY".into(),
    };
    let adapter = OpenAiChatAdapter::new(config).with_api_key("test-key");
    let err = adapter
        .generate_mock_example(&endpoint(), GenerationIntent::Success)
        .await
        .expect_err("error");
    assert!(err.to_string().contains("429"));
    assert!(err.to_string().contains("rate_limited"));
}
