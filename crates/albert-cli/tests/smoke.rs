use albert_cli::args::Command;
use albert_cli::{CliArgs, RunOutcome, parse_args, run_with_args};
use std::fs;
use tempfile::TempDir;

const OPENAPI: &str = r#"
{
  "openapi": "3.0.3",
  "info": { "title": "CLI Smoke", "version": "0.1.0" },
  "paths": {
    "/ping": {
      "get": {
        "responses": {
          "200": { "description": "ok" }
        }
      }
    }
  }
}
"#;

#[tokio::test]
async fn import_list_serve_export_roundtrip() {
    let temp = TempDir::new().expect("tempdir");
    let db_path = temp.path().join("albert.db");
    let openapi_path = temp.path().join("spec.json");
    fs::write(&openapi_path, OPENAPI).unwrap();

    // 1. import
    let args = parse_args([
        "import".to_string(),
        "--db".to_string(),
        db_path.to_string_lossy().to_string(),
        openapi_path.to_string_lossy().to_string(),
    ])
    .unwrap();
    assert_eq!(args.command, Command::Import);
    let out = run_with_args(args).await.expect("import");
    match out {
        RunOutcome::Message(msg) => assert!(msg.contains("imported")),
        _ => panic!("unexpected outcome"),
    }

    // 2. list
    let args = parse_args([
        "list".to_string(),
        "--db".to_string(),
        db_path.to_string_lossy().to_string(),
    ])
    .unwrap();
    let out = run_with_args(args).await.expect("list");
    let listing = match out {
        RunOutcome::Message(msg) => msg,
        _ => panic!("unexpected outcome"),
    };
    assert!(listing.contains("CLI Smoke") || listing.contains("spec"));

    // 3. discover collection id via list_collections directly for export
    let store = albert_storage::SqliteStore::new(db_path.to_string_lossy().to_string());
    store.migrate().unwrap();
    let summaries = store.list_collections().unwrap();
    assert_eq!(summaries.len(), 1);
    let collection_id = summaries[0].id.clone();

    // 4. export to file
    let export_path = temp.path().join("export.json");
    let args = parse_args([
        "export".to_string(),
        "--db".to_string(),
        db_path.to_string_lossy().to_string(),
        "--id".to_string(),
        collection_id.clone(),
        "--output".to_string(),
        export_path.to_string_lossy().to_string(),
    ])
    .unwrap();
    let out = run_with_args(args).await.expect("export");
    match out {
        RunOutcome::Message(msg) => assert!(msg.contains("wrote")),
        _ => panic!("unexpected outcome"),
    }
    let contents = fs::read_to_string(&export_path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&contents).unwrap();
    assert_eq!(parsed["id"], collection_id);

    // 5. serve with auto-stop, ensure /ping responds
    // Run the server task, then hit it after a short sleep.
    let args = parse_args([
        "serve".to_string(),
        "--db".to_string(),
        db_path.to_string_lossy().to_string(),
        "--host".to_string(),
        "127.0.0.1".to_string(),
        "--port".to_string(),
        "0".to_string(),
        "--auto-stop-secs".to_string(),
        "2".to_string(),
    ])
    .unwrap();
    let CliArgs { .. } = args; // ensure it's shaped correctly

    // For the smoke test we bypass ctrl-c by invoking the gateway directly,
    // because the chosen port (0) resolves only after binding.
    let store = albert_storage::SqliteStore::new(db_path.to_string_lossy().to_string());
    store.migrate().unwrap();
    let collections = store.load_all_collections().unwrap();
    let gateway = albert_gateway::MockGateway::new();
    let status = gateway
        .start(
            collections,
            albert_gateway::GatewayConfig {
                port: 0,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let bind = status.bind_address.clone().unwrap();
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("http://{bind}/ping"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 200);
    gateway.stop().await.unwrap();
}

#[tokio::test]
async fn watch_picks_up_file_changes() {
    let temp = TempDir::new().expect("tempdir");
    let db_path = temp.path().join("albert.db");
    let openapi_path = temp.path().join("spec.json");
    fs::write(&openapi_path, OPENAPI).unwrap();

    // Kick off `watch` with a short interval and auto-stop after 2s.
    let args = parse_args([
        "watch".to_string(),
        "--db".to_string(),
        db_path.to_string_lossy().to_string(),
        "--interval-ms".to_string(),
        "150".to_string(),
        "--auto-stop-secs".to_string(),
        "2".to_string(),
        openapi_path.to_string_lossy().to_string(),
    ])
    .unwrap();

    let watch_task = tokio::spawn(async move { run_with_args(args).await });

    // After ~600ms the initial import should be reflected in the store.
    tokio::time::sleep(std::time::Duration::from_millis(700)).await;
    let store = albert_storage::SqliteStore::new(db_path.to_string_lossy().to_string());
    store.migrate().unwrap();
    assert_eq!(store.list_collections().unwrap().len(), 1);

    // Modify the spec to add a new endpoint + bump mtime by rewriting the file.
    let modified_spec = r#"
{
  "openapi": "3.0.3",
  "info": { "title": "CLI Smoke", "version": "0.2.0" },
  "paths": {
    "/ping": { "get": { "responses": { "200": { "description": "ok" } } } },
    "/pong": { "get": { "responses": { "200": { "description": "ok" } } } }
  }
}
"#;
    // Sleep briefly to ensure the filesystem's mtime resolution registers
    // the change; some platforms track at 1s granularity.
    tokio::time::sleep(std::time::Duration::from_millis(1_100)).await;
    fs::write(&openapi_path, modified_spec).unwrap();

    // Let auto-stop fire; watch_task resolves when the deadline is reached.
    let outcome = watch_task
        .await
        .expect("watch task panic")
        .expect("watch result");
    match outcome {
        RunOutcome::Message(msg) => assert_eq!(msg, "watch stopped"),
        other => panic!("unexpected outcome: {other:?}"),
    }

    // The change should be reflected: endpoints count is 2 after the
    // second import.
    let summaries = store.list_collections().unwrap();
    assert_eq!(summaries.len(), 1);
    let collection = store
        .load_collection(&summaries[0].id)
        .unwrap()
        .expect("collection");
    assert_eq!(collection.endpoints.len(), 2);
}

#[tokio::test]
async fn help_and_version_return_messages() {
    let args = parse_args(["help".to_string()]).unwrap();
    let out = run_with_args(args).await.unwrap();
    match out {
        RunOutcome::Message(msg) => {
            assert!(msg.contains("USAGE"));
            assert!(msg.contains("serve"));
        }
        _ => panic!("expected help message"),
    }

    let args = parse_args(["version".to_string()]).unwrap();
    let out = run_with_args(args).await.unwrap();
    match out {
        RunOutcome::Message(msg) => assert!(msg.starts_with("albert ")),
        _ => panic!("expected version message"),
    }
}
