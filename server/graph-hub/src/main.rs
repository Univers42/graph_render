//! The `graph-hub` command. No argument serves; `healthcheck` probes `/healthz` on
//! `127.0.0.1:$GRAPH_HUB_PORT`. Exit 2 is a refusal to start, and the message names the variable or
//! the file, never a value.

use graph_hub::app::App;
use graph_hub::{health, serve};
use std::net::{IpAddr, SocketAddr};
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

/// Task 1 serves what exists: the state, the two routes of the table and §6's connection limits
/// read straight from the environment. Task 2 makes it `Settings::from_env` then
/// `Settings::check`, so this is the shape `main` converges on, not a parallel path.
fn serve_from_env() -> ExitCode {
    let log = graph_hub::observe::stdout_sink();
    let app = App::new(log);
    serve::run(addr_from_env(), serve::Limits::default(), app)
}

/// `GRAPH_HUB_BIND`/`GRAPH_HUB_PORT`, read here for Task 1 and by `config::Settings::from_env`
/// from Task 2 on. Caveat: an unparsable address refuses the start with exit 2, naming the
/// variable and never its value, which is the same refusal `ConfigError` makes.
fn addr_from_env() -> SocketAddr {
    let bind = match std::env::var("GRAPH_HUB_BIND") {
        Ok(text) => match text.parse::<IpAddr>() {
            Ok(bind) => bind,
            Err(_) => refuse_ip("GRAPH_HUB_BIND: is malformed"),
        },
        Err(_) => IpAddr::from([127, 0, 0, 1]),
    };
    let port = match std::env::var("GRAPH_HUB_PORT") {
        Ok(text) => match text.parse::<u16>() {
            Ok(port) => port,
            Err(_) => refuse_ip("GRAPH_HUB_PORT: is malformed"),
        },
        Err(_) => 8080,
    };
    SocketAddr::new(bind, port)
}

fn refuse_ip(message: &str) -> ! {
    eprintln!("graph-hub: {message}");
    std::process::exit(2)
}

fn healthcheck() -> ExitCode {
    let port = match std::env::var("GRAPH_HUB_PORT") {
        Ok(text) => match text.parse::<u16>() {
            Ok(port) => port,
            Err(_) => return refuse("GRAPH_HUB_PORT: is malformed"),
        },
        Err(_) => 8080,
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
