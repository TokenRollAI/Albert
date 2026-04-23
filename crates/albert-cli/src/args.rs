//! Minimal hand-rolled argument parser so the CLI has no external deps.

use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Serve,
    Import,
    Watch,
    List,
    Export,
    ExportAll,
    Delete,
    Rename,
    Doctor,
    Help,
    Version,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CliArgs {
    pub command: Command,
    pub database_url: String,
    pub host: String,
    pub port: u16,
    pub cors: bool,
    pub default_latency_ms: Option<u64>,
    pub error_rate: f32,
    pub collections: Vec<String>,
    /// Files to import when `command == Import`.
    pub import_paths: Vec<PathBuf>,
    /// Target file to write in `export` mode; stdout when `None`.
    pub export_output: Option<PathBuf>,
    /// Single collection id to target for `export`.
    pub export_collection_id: Option<String>,
    /// Stop the server automatically after N seconds (test harness).
    pub auto_stop_secs: Option<u64>,
    /// Capture request bodies (≤4KB) into the request log.
    pub capture_bodies: bool,
    /// New name for rename command.
    pub new_name: Option<String>,
    /// Watch poll interval in seconds (defaults to 1.0).
    pub watch_interval_ms: Option<u64>,
}

impl Default for CliArgs {
    fn default() -> Self {
        Self {
            command: Command::Help,
            database_url: "albert.db".to_string(),
            host: "127.0.0.1".to_string(),
            port: 4317,
            cors: true,
            default_latency_ms: None,
            error_rate: 0.0,
            collections: Vec::new(),
            import_paths: Vec::new(),
            export_output: None,
            export_collection_id: None,
            auto_stop_secs: None,
            capture_bodies: false,
            new_name: None,
            watch_interval_ms: None,
        }
    }
}

#[derive(Debug, Error)]
pub enum CliError {
    #[error("unknown command: {0}")]
    UnknownCommand(String),
    #[error("unknown flag: {0}")]
    UnknownFlag(String),
    #[error("flag --{flag} requires a value")]
    MissingValue { flag: String },
    #[error("could not parse --{flag}={value}: {reason}")]
    BadValue {
        flag: String,
        value: String,
        reason: String,
    },
}

pub fn parse_args<I, S>(iter: I) -> Result<CliArgs, CliError>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut argv: Vec<String> = iter.into_iter().map(Into::into).collect();
    let mut out = CliArgs::default();

    if argv.is_empty() {
        return Ok(CliArgs {
            command: Command::Help,
            ..out
        });
    }

    let first = argv.remove(0);
    out.command = match first.as_str() {
        "serve" => Command::Serve,
        "import" => Command::Import,
        "watch" => Command::Watch,
        "list" => Command::List,
        "export" => Command::Export,
        "export-all" => Command::ExportAll,
        "delete" => Command::Delete,
        "rename" => Command::Rename,
        "doctor" => Command::Doctor,
        "help" | "--help" | "-h" => Command::Help,
        "version" | "--version" | "-V" => Command::Version,
        other => return Err(CliError::UnknownCommand(other.to_string())),
    };

    let mut i = 0;
    while i < argv.len() {
        let arg = &argv[i];
        let (flag, inline_value) = if let Some(rest) = arg.strip_prefix("--") {
            match rest.split_once('=') {
                Some((k, v)) => (k.to_string(), Some(v.to_string())),
                None => (rest.to_string(), None),
            }
        } else {
            match out.command {
                Command::Import | Command::Watch => {
                    out.import_paths.push(PathBuf::from(arg));
                    i += 1;
                    continue;
                }
                _ => return Err(CliError::UnknownFlag(arg.clone())),
            }
        };

        let take_value = |i: &mut usize| -> Result<String, CliError> {
            if let Some(v) = inline_value.clone() {
                return Ok(v);
            }
            *i += 1;
            if *i >= argv.len() {
                return Err(CliError::MissingValue { flag: flag.clone() });
            }
            Ok(argv[*i].clone())
        };

        match flag.as_str() {
            "db" | "database" => {
                out.database_url = take_value(&mut i)?;
            }
            "host" => {
                out.host = take_value(&mut i)?;
            }
            "port" => {
                let v = take_value(&mut i)?;
                out.port = v.parse::<u16>().map_err(|err| CliError::BadValue {
                    flag: flag.clone(),
                    value: v,
                    reason: err.to_string(),
                })?;
            }
            "no-cors" => {
                out.cors = false;
            }
            "default-latency-ms" => {
                let v = take_value(&mut i)?;
                let parsed = v.parse::<u64>().map_err(|err| CliError::BadValue {
                    flag: flag.clone(),
                    value: v.clone(),
                    reason: err.to_string(),
                })?;
                out.default_latency_ms = if parsed == 0 { None } else { Some(parsed) };
            }
            "error-rate" => {
                let v = take_value(&mut i)?;
                let parsed = v.parse::<f32>().map_err(|err| CliError::BadValue {
                    flag: flag.clone(),
                    value: v.clone(),
                    reason: err.to_string(),
                })?;
                out.error_rate = parsed.clamp(0.0, 1.0);
            }
            "collection" | "c" => {
                out.collections.push(take_value(&mut i)?);
            }
            "output" | "o" => {
                out.export_output = Some(PathBuf::from(take_value(&mut i)?));
            }
            "id" => {
                out.export_collection_id = Some(take_value(&mut i)?);
            }
            "auto-stop-secs" => {
                let v = take_value(&mut i)?;
                let parsed = v.parse::<u64>().map_err(|err| CliError::BadValue {
                    flag: flag.clone(),
                    value: v.clone(),
                    reason: err.to_string(),
                })?;
                out.auto_stop_secs = Some(parsed);
            }
            "capture-bodies" => {
                out.capture_bodies = true;
            }
            "name" => {
                out.new_name = Some(take_value(&mut i)?);
            }
            "interval-ms" => {
                let v = take_value(&mut i)?;
                let parsed = v.parse::<u64>().map_err(|err| CliError::BadValue {
                    flag: flag.clone(),
                    value: v.clone(),
                    reason: err.to_string(),
                })?;
                out.watch_interval_ms = Some(parsed.max(100));
            }
            "help" | "h" => {
                out.command = Command::Help;
            }
            "version" | "V" => {
                out.command = Command::Version;
            }
            other => return Err(CliError::UnknownFlag(other.to_string())),
        }
        i += 1;
    }

    Ok(out)
}

pub fn help_text() -> String {
    let mut s = String::new();
    s.push_str("Albert CLI — headless mock gateway driver\n\n");
    s.push_str("USAGE:\n");
    s.push_str("    albert <COMMAND> [OPTIONS]\n\n");
    s.push_str("COMMANDS:\n");
    s.push_str("    serve      Start the mock HTTP gateway\n");
    s.push_str("    import     Import an OpenAPI/cURL file into the SQLite store\n");
    s.push_str("    watch      Re-import a file on every change (Ctrl-C to stop)\n");
    s.push_str("    list       List collections stored in the database\n");
    s.push_str("    export     Print a collection snapshot as JSON\n");
    s.push_str("    export-all Print all collections as a JSON array\n");
    s.push_str("    delete     Remove a collection from the database\n");
    s.push_str("    rename     Rename an existing collection\n");
    s.push_str("    doctor     Run health checks (db, env, provider reachability)\n");
    s.push_str("    help       Print this help\n");
    s.push_str("    version    Print the crate version\n\n");
    s.push_str("SHARED OPTIONS:\n");
    s.push_str("    --db <path>              SQLite database path (default: albert.db)\n\n");
    s.push_str("SERVE OPTIONS:\n");
    s.push_str("    --host <ip>              Bind address (default: 127.0.0.1)\n");
    s.push_str("    --port <n>               Bind port (default: 4317, 0 = ephemeral)\n");
    s.push_str("    --no-cors                Disable permissive CORS\n");
    s.push_str("    --default-latency-ms <n> Add a latency floor to every route\n");
    s.push_str("    --error-rate <0..1>      Chance of serving the error example\n");
    s.push_str("    --collection <id>        Only serve the named collection(s)\n");
    s.push_str("    --capture-bodies         Record request bodies in the log (≤4KB)\n");
    s.push_str("    --auto-stop-secs <n>     Stop after N seconds (useful in tests)\n\n");
    s.push_str("DELETE OPTIONS:\n");
    s.push_str("    --id <collection_id>     Collection to remove\n\n");
    s.push_str("WATCH OPTIONS:\n");
    s.push_str("    <file>                   Path to watch (positional, required)\n");
    s.push_str("    --interval-ms <n>        Poll interval in ms (default 1000, min 100)\n");
    s.push_str("    --auto-stop-secs <n>     Exit after N seconds (useful in tests)\n\n");
    s.push_str("RENAME OPTIONS:\n");
    s.push_str("    --id <collection_id>     Collection to rename\n");
    s.push_str("    --name <new_name>        New display name\n\n");
    s.push_str("EXPORT OPTIONS:\n");
    s.push_str("    --id <collection_id>     Collection to export\n");
    s.push_str("    --output <path>          File to write (default: stdout)\n");
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_to_help_without_command() {
        let args = parse_args(Vec::<String>::new()).unwrap();
        assert_eq!(args.command, Command::Help);
    }

    #[test]
    fn parses_serve_with_flags() {
        let args = parse_args([
            "serve",
            "--db",
            "/tmp/albert.db",
            "--port",
            "8080",
            "--no-cors",
            "--default-latency-ms",
            "50",
            "--error-rate=0.25",
            "--collection",
            "api",
        ])
        .unwrap();
        assert_eq!(args.command, Command::Serve);
        assert_eq!(args.database_url, "/tmp/albert.db");
        assert_eq!(args.port, 8080);
        assert!(!args.cors);
        assert_eq!(args.default_latency_ms, Some(50));
        assert!((args.error_rate - 0.25).abs() < 1e-4);
        assert_eq!(args.collections, vec!["api".to_string()]);
    }

    #[test]
    fn parses_import_with_positional_files() {
        let args = parse_args(["import", "--db", "db.sqlite", "a.json", "b.yaml"]).unwrap();
        assert_eq!(args.command, Command::Import);
        assert_eq!(args.import_paths.len(), 2);
    }

    #[test]
    fn rejects_unknown_flag() {
        let err = parse_args(["serve", "--cosmic"]).unwrap_err();
        assert!(matches!(err, CliError::UnknownFlag(_)));
    }
}
