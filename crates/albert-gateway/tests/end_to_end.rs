//! End-to-end integration test: parse a real OpenAPI document, persist it via
//! albert-storage, reload it, start the gateway, and hit every route.

use albert_gateway::{GatewayConfig, MockGateway};
use albert_parser::{ParseSource, parse_source};
use albert_storage::SqliteStore;
use tempfile::TempDir;

const OPENAPI: &str = r#"
{
  "openapi": "3.0.3",
  "info": { "title": "Albert E2E", "version": "0.1.0" },
  "paths": {
    "/orders": {
      "get": {
        "summary": "List orders",
        "responses": { "200": { "description": "ok" } }
      },
      "post": {
        "summary": "Create order",
        "responses": { "201": { "description": "created" } }
      }
    },
    "/orders/{id}": {
      "get": {
        "summary": "Get order",
        "parameters": [
          { "name": "id", "in": "path", "required": true, "schema": {"type": "string"} }
        ],
        "responses": { "200": { "description": "ok" } }
      }
    }
  }
}
"#;

#[tokio::test]
async fn parse_persist_reload_serve_roundtrip() {
    let temp = TempDir::new().expect("tempdir");
    let db_path = temp.path().join("albert.db");
    let db_url = db_path.to_string_lossy().to_string();

    let collection = parse_source(ParseSource {
        name: Some("Albert E2E".into()),
        body: OPENAPI.to_string(),
    })
    .expect("parse");
    assert_eq!(collection.endpoints.len(), 3);

    let store = SqliteStore::new(db_url);
    store.migrate().expect("migrate");
    store.save_collection(&collection).expect("save");

    let reloaded = store
        .load_collection(&collection.id)
        .expect("load")
        .expect("exists");
    assert_eq!(reloaded.endpoints.len(), 3);

    let gateway = MockGateway::new();
    let status = gateway
        .start(
            vec![reloaded.clone()],
            GatewayConfig {
                port: 0,
                ..Default::default()
            },
        )
        .await
        .expect("start");
    let bind = status.bind_address.clone().expect("bind");
    let base = format!("http://{bind}");

    let client = reqwest::Client::new();

    let resp = client
        .get(format!("{base}/orders"))
        .send()
        .await
        .expect("GET /orders");
    assert_eq!(resp.status().as_u16(), 200);
    assert_eq!(
        resp.headers()
            .get("x-albert-mock-kind")
            .and_then(|v| v.to_str().ok()),
        Some("success")
    );

    let resp = client
        .post(format!("{base}/orders"))
        .json(&serde_json::json!({"total": 99}))
        .send()
        .await
        .expect("POST /orders");
    assert_eq!(resp.status().as_u16(), 200);

    let resp = client
        .get(format!("{base}/orders/42"))
        .send()
        .await
        .expect("GET /orders/42");
    assert_eq!(resp.status().as_u16(), 200);
    assert_eq!(
        resp.headers()
            .get("x-albert-mock-route")
            .and_then(|v| v.to_str().ok()),
        Some("GET /orders/{id}")
    );

    let resp = client
        .get(format!("{base}/orders?__albert_mock=error"))
        .send()
        .await
        .expect("error override");
    assert_eq!(resp.status().as_u16(), 400);

    let resp = client
        .get(format!("{base}/does-not-exist"))
        .send()
        .await
        .expect("404");
    assert_eq!(resp.status().as_u16(), 404);

    gateway.stop().await.expect("stop");
}
