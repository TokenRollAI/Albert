//! Top-level command dispatch.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::time::Duration;

use albert_gateway::{GatewayConfig, GatewayStatus, MockGateway};
use albert_storage::SqliteStore;

use crate::args::{CliArgs, Command, help_text};
use crate::ingest::ingest_file;

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
        Command::Serve => run_serve(args).await,
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
            Ok(collection) => {
                messages.push(format!(
                    "imported {} ({} endpoints) from {}",
                    collection.name,
                    collection.endpoints.len(),
                    path.display()
                ));
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
