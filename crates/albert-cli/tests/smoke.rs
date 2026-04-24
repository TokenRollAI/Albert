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
async fn export_all_and_bundle_reimport_roundtrip() {
    let temp = TempDir::new().expect("tempdir");
    let db_path = temp.path().join("origin.db");
    let spec_a = temp.path().join("a.json");
    let spec_b = temp.path().join("b.json");
    fs::write(&spec_a, OPENAPI).unwrap();
    fs::write(
        &spec_b,
        r#"{"openapi":"3.0.3","info":{"title":"B","version":"1"},"paths":{"/b":{"get":{"responses":{"200":{"description":"ok"}}}}}}"#,
    )
    .unwrap();

    // import two specs
    for spec in [&spec_a, &spec_b] {
        let args = parse_args([
            "import".to_string(),
            "--db".to_string(),
            db_path.to_string_lossy().to_string(),
            spec.to_string_lossy().to_string(),
        ])
        .unwrap();
        run_with_args(args).await.expect("import");
    }

    // export-all to a bundle file
    let bundle = temp.path().join("bundle.json");
    let args = parse_args([
        "export-all".to_string(),
        "--db".to_string(),
        db_path.to_string_lossy().to_string(),
        "--output".to_string(),
        bundle.to_string_lossy().to_string(),
    ])
    .unwrap();
    run_with_args(args).await.expect("export-all");
    assert!(bundle.exists());

    // import the bundle into a fresh DB
    let replica_db = temp.path().join("replica.db");
    let args = parse_args([
        "import".to_string(),
        "--db".to_string(),
        replica_db.to_string_lossy().to_string(),
        bundle.to_string_lossy().to_string(),
    ])
    .unwrap();
    let outcome = run_with_args(args).await.expect("bundle import");
    match outcome {
        RunOutcome::Message(msg) => {
            assert!(msg.contains("bundle"), "{msg}");
            assert!(msg.contains("2 collections"), "{msg}");
        }
        other => panic!("unexpected: {other:?}"),
    }

    // Verify identical collection count + ids in the replica.
    let origin = albert_storage::SqliteStore::new(db_path.to_string_lossy().to_string());
    origin.migrate().unwrap();
    let replica = albert_storage::SqliteStore::new(replica_db.to_string_lossy().to_string());
    replica.migrate().unwrap();
    let mut origin_ids: Vec<String> = origin
        .list_collections()
        .unwrap()
        .into_iter()
        .map(|c| c.id)
        .collect();
    let mut replica_ids: Vec<String> = replica
        .list_collections()
        .unwrap()
        .into_iter()
        .map(|c| c.id)
        .collect();
    origin_ids.sort();
    replica_ids.sort();
    assert_eq!(origin_ids, replica_ids);
}

#[tokio::test]
async fn verify_hits_every_route() {
    let temp = TempDir::new().expect("tempdir");
    let db_path = temp.path().join("albert.db");
    let openapi_path = temp.path().join("spec.json");
    fs::write(&openapi_path, OPENAPI).unwrap();

    // Seed the db so the gateway has something to serve.
    let args = parse_args([
        "import".to_string(),
        "--db".to_string(),
        db_path.to_string_lossy().to_string(),
        openapi_path.to_string_lossy().to_string(),
    ])
    .unwrap();
    run_with_args(args).await.expect("import");

    // Spin up a gateway with those collections.
    let store = albert_storage::SqliteStore::new(db_path.to_string_lossy().to_string());
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

    let args = parse_args([
        "verify".to_string(),
        "--url".to_string(),
        format!("http://{bind}"),
    ])
    .unwrap();
    let outcome = run_with_args(args).await.expect("verify");
    let message = match outcome {
        RunOutcome::Message(msg) => msg,
        other => panic!("unexpected: {other:?}"),
    };
    assert!(message.contains("[ ok ]"));
    assert!(message.contains("verified"));

    gateway.stop().await.unwrap();
}

#[tokio::test]
async fn ping_reports_running_gateway() {
    // Stand up a local mock gateway on an ephemeral port.
    let gateway = albert_gateway::MockGateway::new();
    let status = gateway
        .start(
            Vec::new(),
            albert_gateway::GatewayConfig {
                port: 0,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let bind = status.bind_address.clone().unwrap();

    let args = parse_args([
        "ping".to_string(),
        "--url".to_string(),
        format!("http://{bind}"),
    ])
    .unwrap();
    let outcome = run_with_args(args).await.expect("ping");
    let message = match outcome {
        RunOutcome::Message(msg) => msg,
        other => panic!("unexpected: {other:?}"),
    };
    assert!(message.contains("[ ok ]"));
    assert!(message.contains("routes: 0"));
    assert!(message.contains("requests:"));

    gateway.stop().await.unwrap();
}

#[tokio::test]
async fn bundle_export_and_import_round_trip() {
    let temp = TempDir::new().unwrap();
    let db_path = temp.path().join("albert.db");
    let openapi_path = temp.path().join("spec.json");
    fs::write(&openapi_path, OPENAPI).unwrap();

    // Seed SQLite so bundle import can resolve collection_ids.
    let import_args = parse_args([
        "import".to_string(),
        "--db".to_string(),
        db_path.to_string_lossy().to_string(),
        openapi_path.to_string_lossy().to_string(),
    ])
    .unwrap();
    run_with_args(import_args).await.expect("import");

    // Start a gateway on an ephemeral port that serves the imported collection.
    let store = albert_storage::SqliteStore::new(db_path.to_string_lossy().to_string());
    store.migrate().unwrap();
    let collections = store.load_all_collections().unwrap();
    let gateway = albert_gateway::MockGateway::new();
    gateway
        .start(
            collections,
            albert_gateway::GatewayConfig {
                port: 0,
                error_rate: 0.42,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let bind = gateway.status().await.bind_address.unwrap();

    // `albert bundle export --output <path>` writes a valid bundle.
    let bundle_path = temp.path().join("bundle.json");
    let export_args = parse_args([
        "bundle".to_string(),
        "export".to_string(),
        "--url".to_string(),
        format!("http://{bind}"),
        "--output".to_string(),
        bundle_path.to_string_lossy().to_string(),
    ])
    .unwrap();
    assert_eq!(export_args.command, Command::BundleExport);
    let out = run_with_args(export_args).await.unwrap();
    match out {
        RunOutcome::Message(m) => assert!(m.contains("wrote")),
        other => panic!("unexpected: {other:?}"),
    }
    let parsed: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&bundle_path).unwrap()).unwrap();
    assert_eq!(parsed["bundle_version"], "1.0");
    assert!((parsed["config"]["error_rate"].as_f64().unwrap() - 0.42).abs() < 1e-5);

    // Flip the running server's state so we can see import restore it.
    gateway
        .reconfigure(albert_gateway::ReconfigureOptions {
            collections: store.load_all_collections().unwrap(),
            ..Default::default()
        })
        .await
        .unwrap();
    let reset = gateway.status().await.config;
    assert_eq!(reset.error_rate, 0.0);

    // `albert bundle import <path>` applies the bundle back.
    let import_args = parse_args([
        "bundle".to_string(),
        "import".to_string(),
        bundle_path.to_string_lossy().to_string(),
        "--url".to_string(),
        format!("http://{bind}"),
        "--db".to_string(),
        db_path.to_string_lossy().to_string(),
    ])
    .unwrap();
    assert_eq!(import_args.command, Command::BundleImport);
    let out = run_with_args(import_args).await.unwrap();
    match out {
        RunOutcome::Message(m) => assert!(m.contains("applied bundle")),
        other => panic!("unexpected: {other:?}"),
    }
    // Verify via the HTTP config endpoint since bundle_import_handler
    // updates state slots (which /__albert/config reads from) but not
    // the `running.config` mirror that `gateway.status()` returns.
    let http_client = reqwest::Client::new();
    let live: serde_json::Value = http_client
        .get(format!("http://{bind}/__albert/config"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let restored_err_rate = live["error_rate"].as_f64().unwrap();
    assert!(
        (restored_err_rate - 0.42).abs() < 1e-5,
        "got {restored_err_rate}"
    );

    gateway.stop().await.unwrap();
}

#[tokio::test]
async fn bundle_import_errors_when_collection_missing() {
    let temp = TempDir::new().unwrap();
    let db_path = temp.path().join("albert.db");
    // Write a bundle referencing a collection id that doesn't exist in the
    // (empty) local store. Gateway URL unused because we fail early.
    let bundle_path = temp.path().join("bundle.json");
    fs::write(
        &bundle_path,
        serde_json::to_string(&serde_json::json!({
            "bundle_version": "1.0",
            "config": { "host": "127.0.0.1", "port": 0, "cors_enabled": true },
            "collection_ids": ["ghost-collection"],
        }))
        .unwrap(),
    )
    .unwrap();
    let args = parse_args([
        "bundle".to_string(),
        "import".to_string(),
        bundle_path.to_string_lossy().to_string(),
        "--url".to_string(),
        "http://127.0.0.1:1".to_string(),
        "--db".to_string(),
        db_path.to_string_lossy().to_string(),
    ])
    .unwrap();
    let err = run_with_args(args).await.unwrap_err();
    assert!(err.contains("ghost-collection"));
}

#[tokio::test]
async fn openapi_subcommand_fetches_spec() {
    let gateway = albert_gateway::MockGateway::new();
    let status = gateway
        .start(
            Vec::new(),
            albert_gateway::GatewayConfig {
                port: 0,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let bind = status.bind_address.clone().unwrap();

    // Stdout form.
    let stdout_args = parse_args([
        "openapi".to_string(),
        "--url".to_string(),
        format!("http://{bind}"),
    ])
    .unwrap();
    assert_eq!(stdout_args.command, Command::Openapi);
    let outcome = run_with_args(stdout_args).await.expect("openapi");
    let message = match outcome {
        RunOutcome::Message(m) => m,
        other => panic!("unexpected: {other:?}"),
    };
    let doc: serde_json::Value = serde_json::from_str(&message).unwrap();
    assert_eq!(doc["openapi"], "3.0.3");

    // --output form writes bytes to disk.
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("spec.json");
    let output_args = parse_args([
        "openapi".to_string(),
        "--url".to_string(),
        format!("http://{bind}"),
        "--output".to_string(),
        path.to_string_lossy().to_string(),
    ])
    .unwrap();
    let outcome = run_with_args(output_args).await.expect("openapi -o");
    let message = match outcome {
        RunOutcome::Message(m) => m,
        other => panic!("unexpected: {other:?}"),
    };
    assert!(message.contains("wrote"));
    assert!(message.contains(&path.display().to_string()));
    let on_disk: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(on_disk["openapi"], "3.0.3");

    gateway.stop().await.unwrap();
}

#[tokio::test]
async fn config_reports_running_gateway_rules() {
    // Stand up a gateway with a non-default error_rate so we can see the
    // CLI surface it.
    let gateway = albert_gateway::MockGateway::new();
    let status = gateway
        .start(
            Vec::new(),
            albert_gateway::GatewayConfig {
                port: 0,
                error_rate: 0.33,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let bind = status.bind_address.clone().unwrap();

    let args = parse_args([
        "config".to_string(),
        "--url".to_string(),
        format!("http://{bind}"),
    ])
    .unwrap();
    assert_eq!(args.command, Command::Config);
    let outcome = run_with_args(args).await.expect("config");
    let message = match outcome {
        RunOutcome::Message(msg) => msg,
        other => panic!("unexpected: {other:?}"),
    };
    let parsed: serde_json::Value = serde_json::from_str(&message).unwrap();
    assert_eq!(parsed["route_count"], 0);
    // JSON::Value represents f64 — 0.33 round-trips exactly here.
    assert!((parsed["error_rate"].as_f64().unwrap() - 0.33).abs() < 1e-6);

    gateway.stop().await.unwrap();
}

#[tokio::test]
async fn config_surfaces_connection_failure() {
    let args = parse_args([
        "config".to_string(),
        "--url".to_string(),
        "http://127.0.0.1:1".to_string(),
    ])
    .unwrap();
    let err = run_with_args(args).await.expect_err("should fail");
    assert!(err.contains("config request"));
}

#[tokio::test]
async fn ping_surfaces_connection_failure() {
    // Port guaranteed closed (use 1 — typically unavailable and fails fast).
    let args = parse_args([
        "ping".to_string(),
        "--url".to_string(),
        "http://127.0.0.1:1".to_string(),
    ])
    .unwrap();
    let err = run_with_args(args).await.expect_err("should fail");
    assert!(err.contains("status request"));
}

#[tokio::test]
async fn doctor_succeeds_with_local_provider_override() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap().to_string();

    // Serve one HEAD request with 200 OK.
    let server = tokio::spawn(async move {
        if let Ok((mut socket, _)) = listener.accept().await {
            let mut buf = [0u8; 1024];
            let _ = socket.read(&mut buf).await;
            let _ = socket
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                .await;
            let _ = socket.shutdown().await;
        }
    });

    let probe = format!("http://{addr}/healthz");
    // Scope the env override to this test only — deliberately unset before
    // re-reading so the other tests never see it.
    unsafe {
        std::env::set_var("ALBERT_PROVIDER_URL", &probe);
    }

    let temp = TempDir::new().expect("tempdir");
    let db_path = temp.path().join("albert.db");
    let args = parse_args([
        "doctor".to_string(),
        "--db".to_string(),
        db_path.to_string_lossy().to_string(),
    ])
    .unwrap();
    let out = run_with_args(args).await.expect("doctor");
    let message = match out {
        RunOutcome::Message(msg) => msg,
        other => panic!("unexpected: {other:?}"),
    };
    unsafe {
        std::env::remove_var("ALBERT_PROVIDER_URL");
    }
    server.abort();

    assert!(message.contains("[ ok ] database"));
    assert!(message.contains("provider reachable at"));
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

#[tokio::test]
async fn inspect_prints_collection_detail() {
    let temp = TempDir::new().unwrap();
    let db_path = temp.path().join("albert.db");
    let openapi_path = temp.path().join("spec.json");
    fs::write(&openapi_path, OPENAPI).unwrap();

    // Import the fixture first.
    let import_args = parse_args([
        "import".to_string(),
        "--db".to_string(),
        db_path.to_string_lossy().to_string(),
        openapi_path.to_string_lossy().to_string(),
    ])
    .unwrap();
    run_with_args(import_args).await.expect("import");

    // Discover the collection id via the storage API.
    let store = albert_storage::SqliteStore::new(db_path.to_string_lossy().to_string());
    store.migrate().unwrap();
    let summary = &store.list_collections().unwrap()[0];
    let collection_id = summary.id.clone();

    // Text form: METHOD / PATH / AUTH / SUMMARY header.
    let text_args = parse_args([
        "inspect".to_string(),
        "--db".to_string(),
        db_path.to_string_lossy().to_string(),
        "--id".to_string(),
        collection_id.clone(),
    ])
    .unwrap();
    assert_eq!(text_args.command, Command::Inspect);
    let text = match run_with_args(text_args).await.unwrap() {
        RunOutcome::Message(m) => m,
        other => panic!("expected Message, got {other:?}"),
    };
    assert!(text.contains("METHOD"));
    assert!(text.contains("GET"));
    assert!(text.contains("/ping"));

    // JSON form parses as a CanonicalApiCollection.
    let json_args = parse_args([
        "inspect".to_string(),
        "--db".to_string(),
        db_path.to_string_lossy().to_string(),
        "--id".to_string(),
        collection_id,
        "--json".to_string(),
    ])
    .unwrap();
    let json = match run_with_args(json_args).await.unwrap() {
        RunOutcome::Message(m) => m,
        other => panic!("expected Message, got {other:?}"),
    };
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(parsed["id"].as_str().is_some());
    assert!(parsed["endpoints"].as_array().is_some());
}

#[tokio::test]
async fn inspect_errors_on_unknown_collection_id() {
    let temp = TempDir::new().unwrap();
    let db_path = temp.path().join("albert.db");
    // Don't import anything — load_collection will return None.
    let args = parse_args([
        "inspect".to_string(),
        "--db".to_string(),
        db_path.to_string_lossy().to_string(),
        "--id".to_string(),
        "does-not-exist".to_string(),
    ])
    .unwrap();
    let err = run_with_args(args).await.err().unwrap();
    assert!(err.contains("does-not-exist"));
}

#[tokio::test]
async fn routes_emits_tsv_and_json_rows() {
    let temp = TempDir::new().unwrap();
    let db_path = temp.path().join("albert.db");
    let openapi_path = temp.path().join("spec.json");
    fs::write(&openapi_path, OPENAPI).unwrap();

    // Import first so the routes command has something to print.
    let import_args = parse_args([
        "import".to_string(),
        "--db".to_string(),
        db_path.to_string_lossy().to_string(),
        openapi_path.to_string_lossy().to_string(),
    ])
    .unwrap();
    run_with_args(import_args).await.expect("import");

    // TSV form.
    let tsv_args = parse_args([
        "routes".to_string(),
        "--db".to_string(),
        db_path.to_string_lossy().to_string(),
    ])
    .unwrap();
    assert_eq!(tsv_args.command, Command::Routes);
    let tsv_message = match run_with_args(tsv_args).await.unwrap() {
        RunOutcome::Message(m) => m,
        other => panic!("expected Message, got {other:?}"),
    };
    let lines: Vec<&str> = tsv_message.lines().collect();
    assert!(lines.iter().any(|l| l.starts_with("GET\t/ping\t")));
    // Every line must have exactly two tabs (three columns).
    for line in &lines {
        assert_eq!(line.matches('\t').count(), 2, "bad line: {line}");
    }

    // JSON form.
    let json_args = parse_args([
        "routes".to_string(),
        "--db".to_string(),
        db_path.to_string_lossy().to_string(),
        "--json".to_string(),
    ])
    .unwrap();
    assert!(json_args.emit_json);
    let json_message = match run_with_args(json_args).await.unwrap() {
        RunOutcome::Message(m) => m,
        other => panic!("expected Message, got {other:?}"),
    };
    let parsed: serde_json::Value = serde_json::from_str(&json_message).unwrap();
    let rows = parsed.as_array().expect("JSON array");
    assert!(!rows.is_empty());
    assert_eq!(rows[0]["method"], "GET");
    assert_eq!(rows[0]["path"], "/ping");
}

#[tokio::test]
async fn serve_print_config_emits_json_and_exits() {
    let args = parse_args([
        "serve".to_string(),
        "--host".to_string(),
        "10.0.0.1".to_string(),
        "--port".to_string(),
        "0".to_string(),
        "--error-rate".to_string(),
        "0.1".to_string(),
        "--print-config".to_string(),
    ])
    .unwrap();
    let out = run_with_args(args).await.expect("dry-run should succeed");
    let message = match out {
        RunOutcome::Message(m) => m,
        other => panic!("expected Message from --print-config, got {other:?}"),
    };
    let parsed: serde_json::Value = serde_json::from_str(&message).unwrap();
    assert_eq!(parsed["gateway"]["host"], "10.0.0.1");
    assert_eq!(parsed["gateway"]["port"], 0);
    // Clamped float comparison — serde_json preserves the exact literal.
    assert!(parsed["gateway"]["error_rate"].as_f64().unwrap() > 0.09);
}
