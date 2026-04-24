//! Top-level command dispatch.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::time::Duration;

use albert_gateway::{GatewayConfig, GatewayStatus, MockGateway};
use albert_storage::SqliteStore;

use crate::args::{CliArgs, Command, help_text};
use crate::ingest::{Ingested, ingest_file};

#[derive(Debug)]
pub enum RunOutcome {
    Message(String),
    // Boxed because GatewayStatus is much larger than the Message variant
    // once it carries the full runtime config (overrides, header gates,
    // rate-limit rules, etc.); keeps the enum pointer-sized on the hot path.
    Served(Box<GatewayStatus>),
}

pub async fn run_with_args(args: CliArgs) -> Result<RunOutcome, String> {
    match args.command {
        Command::Help => Ok(RunOutcome::Message(help_text())),
        Command::Version => Ok(RunOutcome::Message(format!(
            "albert {}",
            env!("CARGO_PKG_VERSION")
        ))),
        Command::Import => run_import(args),
        Command::List => run_list(args),
        Command::Routes => run_routes(args),
        Command::Inspect => run_inspect(args),
        Command::Config => run_config(args).await,
        Command::Openapi => run_openapi(args).await,
        Command::BundleExport => run_bundle_export(args).await,
        Command::BundleImport => run_bundle_import(args).await,
        Command::Export => run_export(args),
        Command::ExportAll => run_export_all(args),
        Command::Delete => run_delete(args),
        Command::Rename => run_rename(args),
        Command::Doctor => run_doctor(args).await,
        Command::Ping => run_ping(args).await,
        Command::Verify => run_verify(args).await,
        Command::Watch => run_watch(args).await,
        Command::Serve => run_serve(args).await,
    }
}

async fn run_verify(args: CliArgs) -> Result<RunOutcome, String> {
    let base = args
        .ping_url
        .clone()
        .unwrap_or_else(|| "http://127.0.0.1:4317".to_string())
        .trim_end_matches('/')
        .to_string();
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| format!("client build: {e}"))?;

    // Pull the registered route list from /__albert/routes.
    let routes_url = format!("{base}/__albert/routes");
    let resp = client
        .get(&routes_url)
        .send()
        .await
        .map_err(|e| format!("routes request to {routes_url} failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!(
            "routes endpoint returned {} at {routes_url}",
            resp.status()
        ));
    }
    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("routes body parse: {e}"))?;
    let routes: Vec<(String, String)> = body
        .get("routes")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|entry| {
                    let method = entry.get("method")?.as_str()?.to_string();
                    let path = entry.get("path")?.as_str()?.to_string();
                    Some((method, path))
                })
                .collect()
        })
        .unwrap_or_default();

    if routes.is_empty() {
        return Ok(RunOutcome::Message(format!(
            "[ ok ] {base} has no routes to verify"
        )));
    }

    let mut passes: u32 = 0;
    let mut failures: Vec<String> = Vec::new();
    let mut lines = Vec::new();

    for (method, path) in &routes {
        // Substitute path-parameter placeholders with a plausible token so
        // the route actually matches. Real parameter validation should be
        // done elsewhere; this just avoids 404s on templated paths.
        let concrete_path = path.replace("{", ":").replace("}", "");
        let concrete_path = concrete_path
            .split('/')
            .map(|seg| {
                if let Some(name) = seg.strip_prefix(':') {
                    // Use a sentinel per-param so logs are easy to read.
                    format!("_{name}")
                } else {
                    seg.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("/");
        let target = format!("{base}{concrete_path}");

        let req = match method.to_ascii_uppercase().as_str() {
            "GET" => client.get(&target),
            "HEAD" => client.head(&target),
            "OPTIONS" => client.request(reqwest::Method::OPTIONS, &target),
            "POST" => client.post(&target).json(&serde_json::json!({})),
            "PUT" => client.put(&target).json(&serde_json::json!({})),
            "PATCH" => client.patch(&target).json(&serde_json::json!({})),
            "DELETE" => client.delete(&target),
            other => {
                failures.push(format!("{other} {path}: unsupported method"));
                continue;
            }
        };

        match req.send().await {
            Ok(resp) => {
                let status = resp.status();
                if status.as_u16() >= 500 {
                    failures.push(format!("{method} {path}: HTTP {status}"));
                    lines.push(format!("[fail] {method} {path} → {status}"));
                } else {
                    passes += 1;
                    lines.push(format!("[ ok ] {method} {path} → {status}"));
                }
            }
            Err(err) => {
                failures.push(format!("{method} {path}: {err}"));
                lines.push(format!("[fail] {method} {path}: {err}"));
            }
        }
    }

    let summary = format!(
        "\nverified {passes}/{total} route(s) against {base}",
        total = routes.len()
    );

    if failures.is_empty() {
        Ok(RunOutcome::Message(format!(
            "{}{summary}",
            lines.join("\n")
        )))
    } else {
        Err(format!(
            "{}{summary}\n\n{} failure(s):\n- {}",
            lines.join("\n"),
            failures.len(),
            failures.join("\n- ")
        ))
    }
}

/// GET /__albert/openapi.json from a running gateway and pretty-print
/// (or write with --output) the returned spec. The server address is
/// passed through as the `?base=` query so the resulting document has a
/// ready-to-use `servers` entry.
async fn run_openapi(args: CliArgs) -> Result<RunOutcome, String> {
    let base = args
        .ping_url
        .clone()
        .unwrap_or_else(|| "http://127.0.0.1:4317".to_string())
        .trim_end_matches('/')
        .to_string();
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| format!("client build: {e}"))?;
    // URL-encode just the `:` and `/` in the base so a proxy doesn't mis-split.
    let encoded_base = base.replace(':', "%3A").replace('/', "%2F");
    let url = format!("{base}/__albert/openapi.json?base={encoded_base}");
    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("openapi request to {url} failed: {e}"))?;
    let status = resp.status();
    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("openapi body parse: {e}"))?;
    if !status.is_success() {
        return Err(format!("openapi endpoint returned {status}: {body}"));
    }
    let rendered = serde_json::to_string_pretty(&body).map_err(|e| format!("serialize: {e}"))?;
    match args.export_output {
        Some(path) => {
            write_file(&path, &rendered)?;
            Ok(RunOutcome::Message(format!(
                "wrote {} bytes to {}",
                rendered.len(),
                path.display()
            )))
        }
        None => Ok(RunOutcome::Message(rendered)),
    }
}

/// GET /__albert/config/bundle from a running gateway and pretty-print
/// (or write with --output) the returned snapshot. The bundle is a
/// superset of /__albert/config's payload — it also carries the
/// `bundle_version` and the `collection_ids` needed to apply the same
/// rules back elsewhere via `albert bundle import`.
async fn run_bundle_export(args: CliArgs) -> Result<RunOutcome, String> {
    let base = args
        .ping_url
        .clone()
        .unwrap_or_else(|| "http://127.0.0.1:4317".to_string())
        .trim_end_matches('/')
        .to_string();
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| format!("client build: {e}"))?;
    let url = format!("{base}/__albert/config/bundle");
    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("bundle request to {url} failed: {e}"))?;
    let status = resp.status();
    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("bundle body parse: {e}"))?;
    if !status.is_success() {
        return Err(format!("bundle endpoint returned {status}: {body}"));
    }
    let rendered = serde_json::to_string_pretty(&body).map_err(|e| format!("serialize: {e}"))?;
    match args.export_output {
        Some(path) => {
            write_file(&path, &rendered)?;
            Ok(RunOutcome::Message(format!(
                "wrote {} bytes to {}",
                rendered.len(),
                path.display()
            )))
        }
        None => Ok(RunOutcome::Message(rendered)),
    }
}

/// Apply a bundle from disk to a running gateway. The CLI reads the
/// file, resolves `collection_ids` against the local SQLite store (same
/// convention as `albert serve --collection`), and POSTs the combined
/// `{bundle, collections}` payload. Missing IDs fail loudly rather than
/// being silently dropped.
async fn run_bundle_import(args: CliArgs) -> Result<RunOutcome, String> {
    let bundle_path = args
        .import_paths
        .first()
        .ok_or("bundle import needs a path argument")?
        .clone();
    let bundle_text = fs::read_to_string(&bundle_path)
        .map_err(|e| format!("read {}: {e}", bundle_path.display()))?;
    let bundle: serde_json::Value = serde_json::from_str(&bundle_text)
        .map_err(|e| format!("parse {}: {e}", bundle_path.display()))?;
    let collection_ids: Vec<String> = bundle
        .get("collection_ids")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    let store = prepare_store(&args.database_url)?;
    let mut collections = Vec::with_capacity(collection_ids.len());
    let mut missing: Vec<String> = Vec::new();
    for id in &collection_ids {
        match store.load_collection(id).map_err(|e| e.to_string())? {
            Some(c) => collections.push(c),
            None => missing.push(id.clone()),
        }
    }
    if !missing.is_empty() {
        return Err(format!(
            "bundle references unknown collections: {} — run `albert import` first",
            missing.join(", ")
        ));
    }

    let base = args
        .ping_url
        .clone()
        .unwrap_or_else(|| "http://127.0.0.1:4317".to_string())
        .trim_end_matches('/')
        .to_string();
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| format!("client build: {e}"))?;
    let url = format!("{base}/__albert/config/bundle");
    let payload = serde_json::json!({
        "bundle": bundle,
        "collections": collections,
    });
    let resp = client
        .post(&url)
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("bundle import to {url} failed: {e}"))?;
    let status = resp.status();
    if !status.is_success() {
        let err_body: serde_json::Value = resp
            .json()
            .await
            .unwrap_or_else(|_| serde_json::json!({"error": "unreadable"}));
        return Err(format!(
            "gateway rejected bundle ({status}): {}",
            err_body
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or(&err_body.to_string())
        ));
    }
    Ok(RunOutcome::Message(format!(
        "applied bundle from {} to {base}",
        bundle_path.display()
    )))
}

/// GET /__albert/config from a running gateway and pretty-print the
/// JSON. Same `--url` convention as `ping` / `verify`. Returns a
/// user-friendly error when the server isn't reachable.
async fn run_config(args: CliArgs) -> Result<RunOutcome, String> {
    let base = args
        .ping_url
        .clone()
        .unwrap_or_else(|| "http://127.0.0.1:4317".to_string())
        .trim_end_matches('/')
        .to_string();
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(|e| format!("client build: {e}"))?;

    let url = format!("{base}/__albert/config");
    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("config request to {url} failed: {e}"))?;
    let status = resp.status();
    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("config body parse: {e}"))?;
    if !status.is_success() {
        return Err(format!("config endpoint returned {status}: {body}"));
    }
    let rendered = serde_json::to_string_pretty(&body).map_err(|e| format!("serialize: {e}"))?;
    Ok(RunOutcome::Message(rendered))
}

async fn run_ping(args: CliArgs) -> Result<RunOutcome, String> {
    let base = args
        .ping_url
        .clone()
        .unwrap_or_else(|| "http://127.0.0.1:4317".to_string())
        .trim_end_matches('/')
        .to_string();
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(|e| format!("client build: {e}"))?;

    let status_url = format!("{base}/__albert/status");
    let status_resp = client
        .get(&status_url)
        .send()
        .await
        .map_err(|e| format!("status request to {status_url} failed: {e}"))?;
    let status_code = status_resp.status();
    let status_body: serde_json::Value = status_resp
        .json()
        .await
        .map_err(|e| format!("status body parse: {e}"))?;
    if !status_code.is_success() {
        return Err(format!(
            "status endpoint returned {status_code}: {status_body}"
        ));
    }

    let metrics_url = format!("{base}/__albert/metrics");
    let metrics_resp = client
        .get(&metrics_url)
        .send()
        .await
        .map_err(|e| format!("metrics request to {metrics_url} failed: {e}"))?;
    let metrics_body: serde_json::Value = metrics_resp
        .json()
        .await
        .map_err(|e| format!("metrics body parse: {e}"))?;

    let route_count = status_body
        .get("route_count")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let total = metrics_body
        .get("total_requests")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let uptime = metrics_body
        .get("uptime_ms")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let avg_latency = metrics_body
        .get("average_latency_ms")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    let message = format!(
        "[ ok ] {base} is up\n       routes: {route_count}\n       requests: {total} (avg {avg_latency}ms)\n       uptime: {uptime}ms"
    );
    Ok(RunOutcome::Message(message))
}

async fn run_doctor(args: CliArgs) -> Result<RunOutcome, String> {
    let mut lines = Vec::new();
    let mut failures: u32 = 0;

    // 1. Database
    match SqliteStore::new(&args.database_url).migrate() {
        Ok(()) => lines.push(format!(
            "[ ok ] database migratable ({})",
            args.database_url
        )),
        Err(err) => {
            failures += 1;
            lines.push(format!("[fail] database migration: {err}"));
        }
    }

    // 2. Env var — we can't know the user's provider without loading their
    // UI state, but we can sanity-check the commonly-used env names.
    for key in ["OPENAI_API_KEY", "ANTHROPIC_API_KEY"] {
        match std::env::var(key) {
            Ok(ref v) if !v.is_empty() => {
                lines.push(format!("[ ok ] {key} is set (len={})", v.len()));
            }
            _ => lines.push(format!("[warn] {key} is not set")),
        }
    }

    // 3. Optional provider reachability: try https://api.openai.com if no
    // collection is specified, else use the first --collection id as a
    // sentinel (users can override by setting ALBERT_PROVIDER_URL).
    let probe_url = std::env::var("ALBERT_PROVIDER_URL")
        .unwrap_or_else(|_| "https://api.openai.com/v1/models".to_string());
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|e| format!("client build failed: {e}"))?;
    match client.head(&probe_url).send().await {
        Ok(resp) => {
            let status = resp.status();
            if status.as_u16() < 500 {
                lines.push(format!(
                    "[ ok ] provider reachable at {probe_url} (HTTP {status})"
                ));
            } else {
                failures += 1;
                lines.push(format!("[fail] provider returned {status} at {probe_url}"));
            }
        }
        Err(err) => {
            failures += 1;
            lines.push(format!("[fail] provider unreachable at {probe_url}: {err}"));
        }
    }

    if failures > 0 {
        Err(format!(
            "{}\n\n{} check(s) failed",
            lines.join("\n"),
            failures
        ))
    } else {
        Ok(RunOutcome::Message(lines.join("\n")))
    }
}

async fn run_watch(args: CliArgs) -> Result<RunOutcome, String> {
    if args.import_paths.is_empty() {
        return Err("watch requires one or more file paths".into());
    }
    let store = prepare_store(&args.database_url)?;
    let interval_ms = args.watch_interval_ms.unwrap_or(1_000);
    let interval = Duration::from_millis(interval_ms);
    let deadline = args
        .auto_stop_secs
        .map(|secs| tokio::time::Instant::now() + Duration::from_secs(secs));

    let mut last_seen: std::collections::HashMap<
        std::path::PathBuf,
        Option<std::time::SystemTime>,
    > = std::collections::HashMap::new();

    // Initial import on startup so the store reflects the current files.
    for path in &args.import_paths {
        process_watch_tick(path, &store, &mut last_seen, true);
    }

    println!(
        "watching {} file(s) every {}ms (Ctrl-C to stop)",
        args.import_paths.len(),
        interval_ms
    );

    loop {
        let sleep_fut = tokio::time::sleep(interval);
        tokio::pin!(sleep_fut);
        tokio::select! {
            _ = &mut sleep_fut => {
                for path in &args.import_paths {
                    process_watch_tick(path, &store, &mut last_seen, false);
                }
                if let Some(deadline) = deadline
                    && tokio::time::Instant::now() >= deadline
                {
                    break;
                }
            }
            _ = tokio::signal::ctrl_c() => {
                break;
            }
        }
    }

    Ok(RunOutcome::Message("watch stopped".to_string()))
}

fn process_watch_tick(
    path: &Path,
    store: &SqliteStore,
    last_seen: &mut std::collections::HashMap<std::path::PathBuf, Option<std::time::SystemTime>>,
    initial: bool,
) {
    let modified = std::fs::metadata(path).and_then(|m| m.modified()).ok();
    let entry_key = path.to_path_buf();
    let previous = last_seen.get(&entry_key).cloned().flatten();
    let changed = initial
        || match (modified, previous) {
            (Some(current), Some(prev)) => current != prev,
            (Some(_), None) => true,
            (None, _) => false,
        };
    if !changed {
        return;
    }
    last_seen.insert(entry_key, modified);
    match ingest_file(path, store) {
        Ok(Ingested {
            collections, kind, ..
        }) => {
            let total: usize = collections.iter().map(|c| c.endpoints.len()).sum();
            match kind {
                crate::ingest::IngestKind::Single => {
                    let c = &collections[0];
                    println!(
                        "[watch] {} imported {} ({} endpoints)",
                        path.display(),
                        c.name,
                        c.endpoints.len()
                    );
                }
                crate::ingest::IngestKind::Bundle => {
                    println!(
                        "[watch] {} imported bundle ({} collections, {} endpoints)",
                        path.display(),
                        collections.len(),
                        total
                    );
                }
            }
        }
        Err(err) => {
            eprintln!("[watch] {} failed: {err}", path.display());
        }
    }
}

fn run_rename(args: CliArgs) -> Result<RunOutcome, String> {
    let id = args
        .export_collection_id
        .as_ref()
        .ok_or("--id <collection_id> is required for rename")?;
    let name = args
        .new_name
        .as_ref()
        .ok_or("--name <new_name> is required for rename")?;
    if name.trim().is_empty() {
        return Err("--name cannot be empty".into());
    }
    let store = prepare_store(&args.database_url)?;
    let renamed = store
        .rename_collection(id, name.trim())
        .map_err(|e| e.to_string())?;
    if renamed {
        Ok(RunOutcome::Message(format!(
            "renamed collection {id} to \"{}\"",
            name.trim()
        )))
    } else {
        Ok(RunOutcome::Message(format!(
            "collection {id} was not present"
        )))
    }
}

fn run_export_all(args: CliArgs) -> Result<RunOutcome, String> {
    let store = prepare_store(&args.database_url)?;
    let collections = store.load_all_collections().map_err(|e| e.to_string())?;
    let rendered =
        serde_json::to_string_pretty(&collections).map_err(|e| format!("serialize: {e}"))?;
    match args.export_output {
        Some(path) => {
            write_file(&path, &rendered)?;
            Ok(RunOutcome::Message(format!(
                "wrote {} bytes to {} ({} collection(s))",
                rendered.len(),
                path.display(),
                collections.len()
            )))
        }
        None => Ok(RunOutcome::Message(rendered)),
    }
}

fn run_delete(args: CliArgs) -> Result<RunOutcome, String> {
    let id = args
        .export_collection_id
        .as_ref()
        .ok_or("--id <collection_id> is required for delete")?;
    let store = prepare_store(&args.database_url)?;
    let removed = store.delete_collection(id).map_err(|e| e.to_string())?;
    if removed {
        Ok(RunOutcome::Message(format!("deleted collection {id}")))
    } else {
        Ok(RunOutcome::Message(format!(
            "collection {id} was not present"
        )))
    }
}

fn prepare_store(database_url: &str) -> Result<SqliteStore, String> {
    let store = SqliteStore::new(database_url);
    store
        .migrate()
        .map_err(|e| format!("migration failed: {e}"))?;
    Ok(store)
}

fn run_import(args: CliArgs) -> Result<RunOutcome, String> {
    if args.import_paths.is_empty() {
        return Err("no input files provided; pass one or more paths after `import`".into());
    }
    let store = prepare_store(&args.database_url)?;
    let mut messages = Vec::new();
    for path in &args.import_paths {
        match ingest_file(path, &store) {
            Ok(ingested) => {
                let count = ingested.collections.len();
                if count == 1 {
                    let c = &ingested.collections[0];
                    messages.push(format!(
                        "imported {} ({} endpoints) from {}",
                        c.name,
                        c.endpoints.len(),
                        path.display()
                    ));
                } else {
                    let total_endpoints: usize =
                        ingested.collections.iter().map(|c| c.endpoints.len()).sum();
                    messages.push(format!(
                        "imported bundle from {} ({} collections, {} endpoints)",
                        path.display(),
                        count,
                        total_endpoints
                    ));
                }
            }
            Err(err) => {
                messages.push(format!("failed to import {}: {err}", path.display()));
            }
        }
    }
    Ok(RunOutcome::Message(messages.join("\n")))
}

fn run_list(args: CliArgs) -> Result<RunOutcome, String> {
    let store = prepare_store(&args.database_url)?;
    let collections = store.list_collections().map_err(|e| e.to_string())?;
    if collections.is_empty() {
        return Ok(RunOutcome::Message(format!(
            "no collections in {}",
            args.database_url
        )));
    }
    let mut lines = Vec::new();
    for summary in collections {
        lines.push(format!(
            "{:<30}  {:<8}  {:>3} endpoints    id={}",
            summary.name, summary.source_kind, summary.endpoint_count, summary.id
        ));
    }
    Ok(RunOutcome::Message(lines.join("\n")))
}

fn run_inspect(args: CliArgs) -> Result<RunOutcome, String> {
    let store = prepare_store(&args.database_url)?;
    let id = args
        .export_collection_id
        .as_ref()
        .ok_or("--id <collection_id> is required for inspect")?;
    let collection = store
        .load_collection(id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("collection '{id}' not found"))?;

    if args.emit_json {
        let rendered =
            serde_json::to_string_pretty(&collection).map_err(|e| format!("serialize: {e}"))?;
        return Ok(RunOutcome::Message(rendered));
    }

    // Human-friendly table. Keep the header + every row aligned on a
    // single space separator so users can pipe the output into `less -S`
    // without losing readability.
    let mut lines = Vec::new();
    lines.push(format!(
        "# {} ({})  id={}",
        collection.name,
        collection.source.as_str(),
        collection.id
    ));
    if let Some(description) = &collection.description
        && !description.is_empty()
    {
        lines.push(format!("  {description}"));
    }
    lines.push(format!("  {} endpoint(s):", collection.endpoints.len()));
    lines.push(String::new());
    lines.push(format!(
        "{:<7} {:<40} {:<6} {:<30}",
        "METHOD", "PATH", "AUTH", "SUMMARY"
    ));
    lines.push(format!(
        "{:<7} {:<40} {:<6} {:<30}",
        "------", "----", "----", "-------"
    ));
    for endpoint in &collection.endpoints {
        let auth = match &endpoint.auth {
            Some(hint) => match hint.scheme {
                albert_core::AuthScheme::HttpBearer => "bearer",
                albert_core::AuthScheme::HttpBasic => "basic",
                albert_core::AuthScheme::ApiKeyHeader => "apiKey",
                albert_core::AuthScheme::OAuth2 => "oauth2",
                albert_core::AuthScheme::Other => "other",
            },
            None => "-",
        };
        let summary = endpoint.summary.as_deref().unwrap_or("").to_string();
        let summary_trunc = if summary.len() > 30 {
            format!("{}…", &summary[..29])
        } else {
            summary
        };
        lines.push(format!(
            "{:<7} {:<40} {:<6} {:<30}",
            endpoint.method.as_str(),
            endpoint.path,
            auth,
            summary_trunc
        ));
    }
    Ok(RunOutcome::Message(lines.join("\n")))
}

fn run_routes(args: CliArgs) -> Result<RunOutcome, String> {
    let store = prepare_store(&args.database_url)?;
    let collections = if args.collections.is_empty() {
        store.load_all_collections().map_err(|e| e.to_string())?
    } else {
        let mut out = Vec::new();
        for id in &args.collections {
            if let Some(collection) = store.load_collection(id).map_err(|e| e.to_string())? {
                out.push(collection);
            } else {
                return Err(format!("collection '{id}' not found"));
            }
        }
        out
    };

    if args.emit_json {
        // JSON: [{ method, path, collection, operation_id, summary, auth }].
        let rows: Vec<serde_json::Value> = collections
            .iter()
            .flat_map(|collection| {
                collection.endpoints.iter().map(|endpoint| {
                    serde_json::json!({
                        "method": endpoint.method.as_str(),
                        "path": endpoint.path,
                        "collection": collection.name,
                        "collection_id": collection.id,
                        "operation_id": endpoint.operation_id,
                        "summary": endpoint.summary,
                        "auth": endpoint.auth,
                    })
                })
            })
            .collect();
        let rendered =
            serde_json::to_string_pretty(&rows).map_err(|e| format!("serialize: {e}"))?;
        return Ok(RunOutcome::Message(rendered));
    }

    // TSV: method\tpath\tcollection. Shell-friendly so users can pipe into
    // `awk`, `sort -u`, etc.
    let mut lines = Vec::new();
    for collection in &collections {
        for endpoint in &collection.endpoints {
            lines.push(format!(
                "{}\t{}\t{}",
                endpoint.method.as_str(),
                endpoint.path,
                collection.name
            ));
        }
    }
    if lines.is_empty() {
        return Ok(RunOutcome::Message(format!(
            "no routes in {} — run `albert import <file>` first",
            args.database_url
        )));
    }
    Ok(RunOutcome::Message(lines.join("\n")))
}

fn run_export(args: CliArgs) -> Result<RunOutcome, String> {
    let store = prepare_store(&args.database_url)?;
    let id = args
        .export_collection_id
        .as_ref()
        .ok_or("--id <collection_id> is required for export")?;
    let collection = store
        .load_collection(id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("collection '{id}' not found"))?;
    let rendered =
        serde_json::to_string_pretty(&collection).map_err(|e| format!("serialize: {e}"))?;
    match args.export_output {
        Some(path) => {
            write_file(&path, &rendered)?;
            Ok(RunOutcome::Message(format!(
                "wrote {} bytes to {}",
                rendered.len(),
                path.display()
            )))
        }
        None => Ok(RunOutcome::Message(rendered)),
    }
}

fn write_file(path: &Path, body: &str) -> Result<(), String> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).map_err(|e| format!("mkdir {}: {e}", parent.display()))?;
    }
    fs::write(path, body).map_err(|e| format!("write {}: {e}", path.display()))
}

async fn run_serve(args: CliArgs) -> Result<RunOutcome, String> {
    let print_config = args.print_config;
    let config = GatewayConfig {
        host: args.host.clone(),
        port: args.port,
        cors_enabled: args.cors,
        example_overrides: BTreeMap::new(),
        default_latency_ms: args.default_latency_ms,
        latency_overrides: BTreeMap::new(),
        latency_jitter_ms: BTreeMap::new(),
        error_rate: args.error_rate,
        capture_bodies: args.capture_bodies,
        response_headers: BTreeMap::new(),
        required_headers: BTreeMap::new(),
        rate_limits: BTreeMap::new(),
        status_overrides: BTreeMap::new(),
        enforce_request_bodies: false,
    };

    // Dry-run: dump the resolved config as JSON so users can verify how
    // their flags parsed (especially useful when invoking albert under
    // sudo, npm scripts, or a CI shell that may mangle quoting).
    if print_config {
        let payload = serde_json::json!({
            "database_url": args.database_url,
            "collections": args.collections,
            "gateway": config,
        });
        let rendered =
            serde_json::to_string_pretty(&payload).map_err(|e| format!("serialize config: {e}"))?;
        return Ok(RunOutcome::Message(rendered));
    }

    let store = prepare_store(&args.database_url)?;
    let collections = if args.collections.is_empty() {
        store.load_all_collections().map_err(|e| e.to_string())?
    } else {
        let mut out = Vec::with_capacity(args.collections.len());
        for id in &args.collections {
            if let Some(c) = store.load_collection(id).map_err(|e| e.to_string())? {
                out.push(c);
            } else {
                return Err(format!("collection '{id}' not found"));
            }
        }
        out
    };
    if collections.is_empty() {
        return Err(format!(
            "no collections in {} — run `albert import <file>` first",
            args.database_url
        ));
    }

    let gateway = MockGateway::new();
    let status = gateway
        .start(collections, config)
        .await
        .map_err(|e| format!("start failed: {e}"))?;

    if let Some(secs) = args.auto_stop_secs {
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_secs(secs)) => {}
            _ = tokio::signal::ctrl_c() => {}
        }
    } else {
        tokio::signal::ctrl_c()
            .await
            .map_err(|e| format!("ctrl-c: {e}"))?;
    }
    gateway.stop().await.map_err(|e| format!("stop: {e}"))?;
    Ok(RunOutcome::Served(Box::new(status)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::args::{CliArgs, Command};

    #[tokio::test]
    async fn serve_print_config_dry_runs_without_binding() {
        // --print-config must not require a populated database; that's the
        // whole point of the flag (inspect resolved config, then exit).
        let args = CliArgs {
            command: Command::Serve,
            database_url: "/nonexistent/path/to/nowhere.db".to_string(),
            host: "0.0.0.0".to_string(),
            port: 9876,
            cors: false,
            default_latency_ms: Some(75),
            error_rate: 0.5,
            print_config: true,
            ..Default::default()
        };
        let outcome = run_serve(args).await.expect("dry-run should succeed");
        let message = match outcome {
            RunOutcome::Message(m) => m,
            other => panic!("expected Message, got {other:?}"),
        };
        let parsed: serde_json::Value =
            serde_json::from_str(&message).expect("print-config emits valid JSON");
        assert_eq!(parsed["gateway"]["host"], "0.0.0.0");
        assert_eq!(parsed["gateway"]["port"], 9876);
        assert_eq!(parsed["gateway"]["cors_enabled"], false);
        assert_eq!(parsed["gateway"]["default_latency_ms"], 75);
        assert_eq!(parsed["gateway"]["error_rate"], 0.5);
        assert_eq!(parsed["database_url"], "/nonexistent/path/to/nowhere.db");
    }
}
