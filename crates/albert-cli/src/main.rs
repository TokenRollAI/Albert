use std::process::ExitCode;

use albert_cli::{CliError, RunOutcome, parse_args, run_with_args};

#[tokio::main]
async fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let args = match parse_args(argv) {
        Ok(args) => args,
        Err(err) => {
            print_cli_error(err);
            return ExitCode::from(2);
        }
    };
    match run_with_args(args).await {
        Ok(RunOutcome::Message(msg)) => {
            if !msg.is_empty() {
                println!("{msg}");
            }
            ExitCode::SUCCESS
        }
        Ok(RunOutcome::Served(status)) => {
            if let Some(bind) = status.bind_address {
                println!("mock gateway stopped (was {bind})");
            } else {
                println!("mock gateway stopped");
            }
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("error: {message}");
            ExitCode::from(1)
        }
    }
}

fn print_cli_error(err: CliError) {
    eprintln!("error: {err}");
    eprintln!("run `albert help` for usage");
}
