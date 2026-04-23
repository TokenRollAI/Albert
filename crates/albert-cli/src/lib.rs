//! Headless CLI for running the Albert mock gateway without the Tauri shell.

pub mod args;
pub mod ingest;
pub mod runner;

pub use args::{CliArgs, CliError, parse_args};
pub use ingest::ingest_file;
pub use runner::{RunOutcome, run_with_args};
