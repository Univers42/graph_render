//! The `graph-server` command. No argument serves; `keygen <name>` mints a key; `healthcheck`
//! probes `/healthz` on `127.0.0.1:$GRAPH_PORT`. Exit 2 is a refusal to start.

use graph_server::app::App;
use graph_server::config::{self, Settings};
use graph_server::{health, keys, observe, serve};
use std::process::ExitCode;
use std::sync::Arc;

const USAGE: &str = "usage: graph-server [keygen <name> | healthcheck | --version]";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        [] => serve_from_env(),
        ["keygen", name] => keygen(name),
        ["healthcheck"] => healthcheck(),
        ["--version"] => {
            println!("graph-server {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        _ => refuse(USAGE),
    }
}

fn serve_from_env() -> ExitCode {
    let lookup = |name: &str| std::env::var_os(name);
    let log = observe::stdout_sink();
    log(&config::start_line(&lookup));
    let settings = match Settings::from_env(&lookup) {
        Ok(settings) => settings,
        Err(refused) => return refuse(&refused.to_string()),
    };
    if !settings.auth {
        let warning = "GRAPH_AUTH=off: every request is served without a key; local dev only";
        log(&serde_json::json!({ "event": "warning", "message": warning }).to_string());
    }
    match App::from_settings(&settings, log) {
        Ok(app) => serve::run(&settings, Arc::new(app)),
        Err(refused) => refuse(&refused),
    }
}

/// The key on stdout, once; its file line on stderr, so `2>> keys` appends the line only.
fn keygen(name: &str) -> ExitCode {
    match keys::keygen(name) {
        Ok(minted) => {
            println!("{}", minted.key);
            eprintln!("{}", minted.line);
            ExitCode::SUCCESS
        }
        Err(reason) => refuse(reason),
    }
}

fn healthcheck() -> ExitCode {
    match config::port(&|name| std::env::var_os(name)) {
        Ok(port) if health::healthcheck(port) => ExitCode::SUCCESS,
        Ok(_) => ExitCode::FAILURE,
        Err(refused) => refuse(&refused.to_string()),
    }
}

fn refuse(message: &str) -> ExitCode {
    eprintln!("graph-server: {message}");
    ExitCode::from(2)
}
