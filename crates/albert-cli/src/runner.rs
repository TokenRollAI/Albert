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
    Served(GatewayStatus),
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
        Command::Export => run_export(args),
        Command::ExportAll => run_export_all(args),
        Command::Delete => run_delete(args),
        Command::Rename => run_rename(args),
        Command::Watch => run_watch(args).await,
        Command::Serve => run_serve(args).await,
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

    let config = GatewayConfig {
        host: args.host,
        port: args.port,
        cors_enabled: args.cors,
        example_overrides: BTreeMap::new(),
        default_latency_ms: args.default_latency_ms,
        latency_overrides: BTreeMap::new(),
        error_rate: args.error_rate,
        capture_bodies: args.capture_bodies,
        response_headers: BTreeMap::new(),
    };
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
    Ok(RunOutcome::Served(status))
}
