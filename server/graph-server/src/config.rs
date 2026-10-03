//! Settings, read from the environment only (`docs/contract/service-api.md` "Configuration").
//! An empty variable counts as unset. A malformed one refuses the start (exit 2) with the
//! variable's name and never its value: `GRAPH_API_KEYS_FILE` names a secret's location.

use std::ffi::OsString;
use std::net::IpAddr;
use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;

mod slots;
use slots::cgroup_memory_max;
pub use slots::{BASE_BYTES, PER_SLOT_BYTES, default_workers};

/// Every variable the server reads, in the order the start line lists them.
pub const NAMES: [&str; 14] = [
    "GRAPH_PORT",
    "GRAPH_BIND",
    "GRAPH_API_KEYS_FILE",
    "GRAPH_AUTH",
    "GRAPH_MAX_BODY",
    "GRAPH_WORKERS",
    "GRAPH_QUEUE",
    "GRAPH_TIMEOUT_MS",
    "GRAPH_BODY_TIMEOUT_MS",
    "GRAPH_HEADER_TIMEOUT_MS",
    "GRAPH_MAX_HEADER_BYTES",
    "GRAPH_MAX_CONNECTIONS",
    "GRAPH_CORS_ORIGINS",
    "GRAPH_EMBED_DIR",
];

/// One refused variable: its name and what is wrong, never its value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigError {
    /// The variable.
    pub name: &'static str,
    /// What is wrong with it.
    pub reason: &'static str,
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.name, self.reason)
    }
}

/// How a variable is looked up: the process environment in `main`, a map in tests.
pub type Lookup<'a> = &'a dyn Fn(&str) -> Option<OsString>;

/// The compute limits every `/v1/layout` request is held to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// `GRAPH_MAX_BODY`: the largest body, in bytes, chunked bodies included.
    pub max_body: usize,
    /// `GRAPH_WORKERS`: concurrent runs.
    pub workers: usize,
    /// `GRAPH_QUEUE`: requests allowed to wait for a slot.
    pub queue: usize,
    /// `GRAPH_TIMEOUT_MS`: the wait plus the run, and the shutdown drain.
    pub timeout: Duration,
    /// `GRAPH_BODY_TIMEOUT_MS`: reading the whole body.
    pub body_timeout: Duration,
}

/// The connection limits hyper enforces before a request reaches the router.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Connections {
    /// `GRAPH_HEADER_TIMEOUT_MS`: receiving the request head.
    pub header_timeout: Duration,
    /// `GRAPH_MAX_HEADER_BYTES`: hyper's read buffer, so the largest request head.
    pub max_header_bytes: usize,
    /// `GRAPH_MAX_CONNECTIONS`: open connections; the next one waits in the backlog.
    pub max_connections: usize,
}

/// Everything the server is configured with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    /// `GRAPH_BIND`.
    pub bind: IpAddr,
    /// `GRAPH_PORT`; 0 asks the kernel for a free port.
    pub port: u16,
    /// `GRAPH_API_KEYS_FILE`; `None` only with auth off.
    pub keys_file: Option<PathBuf>,
    /// `GRAPH_AUTH`: false only for local dev, on a loopback bind.
    pub auth: bool,
    /// The compute limits.
    pub limits: Limits,
    /// The connection limits.
    pub connections: Connections,
    /// `GRAPH_CORS_ORIGINS`: the origins allowed on `/v1/`, compared byte for byte.
    pub cors_origins: Vec<String>,
    /// `GRAPH_EMBED_DIR`.
    pub embed_dir: Option<PathBuf>,
}

impl Settings {
    /// Reads and checks every variable.
    pub fn from_env(lookup: Lookup<'_>) -> Result<Self, ConfigError> {
        let env = Env(lookup);
        let bind = env.parsed("GRAPH_BIND", IpAddr::from([127, 0, 0, 1]))?;
        let (auth, keys_file) = read_auth(&env, bind)?;
        Ok(Self {
            bind,
            port: env.number("GRAPH_PORT", 8080, 0..=u16::MAX)?,
            keys_file,
            auth,
            limits: read_limits(&env, cgroup_memory_max())?,
            connections: read_connections(&env)?,
            cors_origins: read_origins(&env)?,
            embed_dir: env.text("GRAPH_EMBED_DIR")?.map(PathBuf::from),
        })
    }
}

/// `GRAPH_PORT` alone, for `healthcheck`.
pub fn port(lookup: Lookup<'_>) -> Result<u16, ConfigError> {
    Env(lookup).number("GRAPH_PORT", 8080, 0..=u16::MAX)
}

/// The start line: each variable's name with `set` or `unset`, never a value.
pub fn start_line(lookup: Lookup<'_>) -> String {
    let mut env = serde_json::Map::new();
    for name in NAMES {
        let set = lookup(name).is_some_and(|value| !value.is_empty());
        env.insert(name.to_owned(), (if set { "set" } else { "unset" }).into());
    }
    let version = env!("CARGO_PKG_VERSION");
    serde_json::json!({ "event": "start", "version": version, "env": env }).to_string()
}

fn read_auth(env: &Env<'_>, bind: IpAddr) -> Result<(bool, Option<PathBuf>), ConfigError> {
    let auth = match env.text("GRAPH_AUTH")?.as_deref() {
        None | Some("on") => true,
        Some("off") => false,
        Some(_) => return Err(refuse("GRAPH_AUTH", "must be `on` or `off`")),
    };
    if !auth && !bind.is_loopback() {
        return Err(refuse(
            "GRAPH_AUTH",
            "`off` refuses a non-loopback GRAPH_BIND",
        ));
    }
    let keys_file = env.text("GRAPH_API_KEYS_FILE")?.map(PathBuf::from);
    if auth && keys_file.is_none() {
        return Err(refuse(
            "GRAPH_API_KEYS_FILE",
            "unset while GRAPH_AUTH is on",
        ));
    }
    Ok((auth, keys_file))
}

fn read_limits(env: &Env<'_>, memory_max: Option<u64>) -> Result<Limits, ConfigError> {
    let workers = read_workers(env, memory_max)?;
    Ok(Limits {
        max_body: env.number("GRAPH_MAX_BODY", 64 << 20, 1..=graph_wasm_max_ingest())?,
        workers,
        queue: env.number("GRAPH_QUEUE", 2 * workers, 0..=65_536)?,
        timeout: env.millis("GRAPH_TIMEOUT_MS", 30_000)?,
        body_timeout: env.millis("GRAPH_BODY_TIMEOUT_MS", 10_000)?,
    })
}

fn read_workers(env: &Env<'_>, memory_max: Option<u64>) -> Result<usize, ConfigError> {
    let cores = std::thread::available_parallelism().map_or(1, usize::from);
    let fallback = default_workers(cores, memory_max);
    if fallback == 0 && env.text("GRAPH_WORKERS")?.is_none() {
        return Err(refuse(
            "GRAPH_WORKERS",
            "unset, and memory.max holds no slot (docs/measurements/service-caps.md)",
        ));
    }
    env.number("GRAPH_WORKERS", fallback, 1..=1024)
}

fn read_connections(env: &Env<'_>) -> Result<Connections, ConfigError> {
    Ok(Connections {
        header_timeout: env.millis("GRAPH_HEADER_TIMEOUT_MS", 5_000)?,
        // hyper refuses a read buffer under 8 KiB.
        max_header_bytes: env.number("GRAPH_MAX_HEADER_BYTES", 16_384, 8_192..=1 << 20)?,
        max_connections: env.number("GRAPH_MAX_CONNECTIONS", 256, 1..=65_536)?,
    })
}

fn read_origins(env: &Env<'_>) -> Result<Vec<String>, ConfigError> {
    let Some(list) = env.text("GRAPH_CORS_ORIGINS")? else {
        return Ok(Vec::new());
    };
    let origins: Vec<String> = list
        .split(',')
        .map(str::trim)
        .filter(|origin| !origin.is_empty())
        .map(str::to_owned)
        .collect();
    for origin in &origins {
        if !is_origin(origin) {
            return Err(refuse(
                "GRAPH_CORS_ORIGINS",
                "each entry is an http(s) origin with no path; a wildcard is refused",
            ));
        }
    }
    Ok(origins)
}

/// A serialized origin: a scheme, a host and an optional port, no path and no wildcard.
fn is_origin(text: &str) -> bool {
    let rest = text
        .strip_prefix("https://")
        .or_else(|| text.strip_prefix("http://"));
    rest.is_some_and(|host| {
        !host.is_empty()
            && host
                .bytes()
                .all(|b| b.is_ascii_graphic() && !matches!(b, b'/' | b'*' | b','))
    })
}

/// The motor's own ingest ceiling (`crates/graph-wasm/src/ingest.rs` `MAX_INGEST_BYTES`, which
/// graph-wasm does not export): the motor refuses a larger document anyway, so a larger
/// `GRAPH_MAX_BODY` would only spend memory.
fn graph_wasm_max_ingest() -> usize {
    1 << 30
}

fn refuse(name: &'static str, reason: &'static str) -> ConfigError {
    ConfigError { name, reason }
}

struct Env<'a>(Lookup<'a>);

impl Env<'_> {
    fn text(&self, name: &'static str) -> Result<Option<String>, ConfigError> {
        match (self.0)(name) {
            None => Ok(None),
            Some(value) if value.is_empty() => Ok(None),
            Some(value) => value
                .into_string()
                .map(Some)
                .map_err(|_| refuse(name, "is not UTF-8")),
        }
    }

    fn parsed<T: FromStr>(&self, name: &'static str, default: T) -> Result<T, ConfigError> {
        self.text(name)?.map_or(Ok(default), |text| {
            text.parse().map_err(|_| refuse(name, "is malformed"))
        })
    }

    fn number<T>(
        &self,
        name: &'static str,
        default: T,
        range: std::ops::RangeInclusive<T>,
    ) -> Result<T, ConfigError>
    where
        T: FromStr + PartialOrd,
    {
        let value = self.parsed(name, default)?;
        if range.contains(&value) {
            Ok(value)
        } else {
            Err(refuse(name, "is out of range"))
        }
    }

    fn millis(&self, name: &'static str, default: u64) -> Result<Duration, ConfigError> {
        self.number(name, default, 1..=600_000)
            .map(Duration::from_millis)
    }
}

#[cfg(test)]
mod tests;
