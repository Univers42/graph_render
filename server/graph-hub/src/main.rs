//! The `graph-hub` command. No argument serves; `healthcheck` probes `/healthz` on
//! `127.0.0.1:$GRAPH_HUB_PORT`. Exit 2 is a refusal to start, and the message names the variable or
//! the file, never a value.

use graph_hub::app::App;
use graph_hub::config::{ConfigError, Settings};
use graph_hub::{health, observe, serve};
use std::net::SocketAddr;
use std::process::ExitCode;

const USAGE: &str = "usage: graph-hub [healthcheck | --version]";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        [] => serve_from_env(),
        ["healthcheck"] => healthcheck(),
        ["--version"] => {
            println!("graph-hub {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        _ => refuse(USAGE),
    }
}

/// Reads the settings, refuses what §6 refuses, opens the store, checks the database and serves.
///
/// The order is fixed and is the whole of §6's "start checks": nothing is bound before every check
/// has passed, so a hub that fails one never answers a request. A refusal is exit 2 with
/// `name: reason` on stderr and never a value.
fn serve_from_env() -> ExitCode {
    let lookup = |name: &str| std::env::var_os(name);
    let log = observe::stdout_sink();
    log(&Settings::start_line(&lookup));
    let settings = match Settings::from_env(&lookup) {
        Ok(settings) => settings,
        Err(refused) => return refuse_config(&refused),
    };
    let runtime = match tokio::runtime::Builder::new_multi_thread().enable_all().build() {
        Ok(runtime) => runtime,
        Err(error) => return fail(&error.to_string()),
    };
    let result = runtime.block_on(start(settings, log));
    runtime.shutdown_timeout(serve::RUNTIME_GRACE);
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(refused) => refuse(&refused),
    }
}

/// The store, the start checks and the listener, in that order.
async fn start(settings: Settings, log: graph_hub::LogSink) -> Result<(), String> {
    let mut store = settings.store.clone();
    store.url = settings.db_url.clone();
    let db = graph_store::Store::connect(&store)
        .await
        .map_err(|error| format!("GRAPH_HUB_DB_URL: {}", error.code()))?;
    db.ping()
        .await
        .map_err(|error| format!("GRAPH_HUB_DB_URL: {}", error.code()))?;
    settings
        .check(&db)
        .await
        .map_err(|refused| refused.to_string())?;
    let addr = SocketAddr::new(settings.bind, settings.port);
    let app = App::from_settings(&settings, log)?;
    serve::serve_forever(addr, &settings.connections, app)
        .await
        .map_err(|error| error.to_string())
}

fn refuse_config(refused: &ConfigError) -> ExitCode {
    refuse(&refused.to_string())
}

fn fail(message: &str) -> ExitCode {
    eprintln!("graph-hub: {message}");
    ExitCode::FAILURE
}

fn healthcheck() -> ExitCode {
    let lookup = |name: &str| std::env::var_os(name);
    let port = match graph_hub::config::port(&lookup) {
        Ok(port) => port,
        Err(refused) => return refuse_config(&refused),
    };
    if health::healthcheck(port) {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn refuse(message: &str) -> ExitCode {
    eprintln!("graph-hub: {message}");
    ExitCode::from(2)
}
