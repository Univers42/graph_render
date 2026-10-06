//! Every environment variable the hub reads, in the order the start line lists them, and the
//! reader that turns one into a typed value.
//!
//! An empty variable counts as unset, exactly as graph-server's reader does
//! (`server/graph-server/src/config.rs:253-262`), so `GRAPH_HUB_MAX_BODY=` is the default and not
//! a malformed value. A refused value names the variable and never its value: `GRAPH_HUB_DB_URL`
//! and `GRAPH_HUB_MOTOR_KEY_FILE` name a secret's location.

use std::ffi::OsString;
use std::net::IpAddr;
use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;

use graph_store::StoreConfig;

use super::{Connections, Gates, Limits, Subscribers, capped, capped_millis};

/// Every variable the hub reads, in the order the start line lists them.
///
/// The first five are the ones §6's table does not bound (where the listener binds, where the
/// database is, and the two credential files H8 names); then §6's twenty-six, and last
/// `GRAPH_HUB_TIMEOUT_MS`, which §6 names inside the `GRAPH_HUB_DB_POOL` row rather than in a row of
/// its own but which `Settings::from_env` reads all the same.
///
/// `GRAPH_HUB_MOTOR_URL` and `GRAPH_HUB_MOTOR_KEY_FILE` are this slice's names for §6's
/// `GRAPH_MOTOR_URL` and `GRAPH_MOTOR_KEY_FILE`: every other name in that table carries the
/// `GRAPH_HUB_` prefix, the motor key is the hub's own credential (Decision 6), and the plan's
/// `hub-upload-timeout` row already passes `GRAPH_HUB_MOTOR_URL`.
pub const NAMES: [&str; 32] = [
    "GRAPH_HUB_BIND",
    "GRAPH_HUB_PORT",
    "GRAPH_HUB_DB_URL",
    "GRAPH_HUB_KEYS_FILE",
    "GRAPH_HUB_GRANTS_FILE",
    "GRAPH_HUB_MAX_BODY",
    "GRAPH_HUB_MAX_BATCH",
    "GRAPH_HUB_MAX_RECORD_BYTES",
    "GRAPH_HUB_MAX_PLUGIN_BYTES",
    "GRAPH_HUB_MAX_DOC_BYTES",
    "GRAPH_HUB_RETAIN",
    "GRAPH_HUB_RETAIN_BYTES",
    "GRAPH_HUB_CHANGES_BYTES",
    "GRAPH_HUB_WRITERS",
    "GRAPH_HUB_READS",
    "GRAPH_HUB_LAYOUTS",
    "GRAPH_HUB_WRITERS_PER_KEY",
    "GRAPH_HUB_BODY_TIMEOUT_MS",
    "GRAPH_HUB_MAX_CONNECTIONS",
    "GRAPH_HUB_HEADER_TIMEOUT_MS",
    "GRAPH_HUB_MAX_HEADER_BYTES",
    "GRAPH_HUB_FETCH_ROWS",
    "GRAPH_HUB_MAX_SUBSCRIBERS",
    "GRAPH_HUB_MAX_SUBSCRIBERS_PER_KEY",
    "GRAPH_HUB_SSE_PAGE",
    "GRAPH_HUB_LAST_SEEN",
    "GRAPH_HUB_DB_POOL",
    "GRAPH_HUB_STREAM_DEADLINE_MS",
    "GRAPH_HUB_MOTOR_TIMEOUT_MS",
    "GRAPH_HUB_MOTOR_URL",
    "GRAPH_HUB_MOTOR_KEY_FILE",
    "GRAPH_HUB_TIMEOUT_MS",
];

/// One refused variable: its name and what is wrong, never its value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigError {
    /// The variable.
    pub name: &'static str,
    /// What is wrong with it.
    pub reason: &'static str,
}

impl ConfigError {
    /// A refusal naming `name` for `reason`.
    pub fn new(name: &'static str, reason: &'static str) -> Self {
        ConfigError { name, reason }
    }
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.name, self.reason)
    }
}

/// How a variable is looked up: the process environment in `main`, a map in tests.
pub type Lookup<'a> = &'a dyn Fn(&str) -> Option<OsString>;

/// The reader over one lookup. Every accessor takes the variable's name as its first argument, so
/// a refusal can name it without a second channel.
pub struct Env<'a>(pub Lookup<'a>);

impl Env<'_> {
    /// The value as text, or `None` when it is unset or empty.
    pub fn text(&self, name: &'static str) -> Result<Option<String>, ConfigError> {
        match (self.0)(name) {
            None => Ok(None),
            Some(value) if value.is_empty() => Ok(None),
            Some(value) => value
                .into_string()
                .map(Some)
                .map_err(|_| refuse(name, "is not UTF-8")),
        }
    }

    /// The value parsed, or `default` when it is unset.
    pub fn parsed<T: FromStr>(&self, name: &'static str, default: T) -> Result<T, ConfigError> {
        self.text(name)?.map_or(Ok(default), |text| {
            text.parse().map_err(|_| refuse(name, "is malformed"))
        })
    }

    /// The value as a number inside `range`.
    ///
    /// Caveat: the range is the only bound; nothing checks a byte cap against the memory the
    /// container actually has, so a `GRAPH_HUB_MAX_BODY` larger than the cgroup asks for an OOM
    /// kill rather than a refusal. §6's `hub-memory` row measures the defaults.
    pub fn number<T>(
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

    /// The value as a duration in milliseconds.
    ///
    /// Caveat: the range is `1..=600_000`, graph-server's own (`config.rs:287-289`), so a
    /// 10-minute ceiling applies to every duration here: a longer one is a config change with a
    /// review, not a bigger number. `GRAPH_HUB_IDEM_TTL_MS` is deliberately not read through this
    /// accessor, because its default alone is 24 hours.
    pub fn millis(&self, name: &'static str, default: u64) -> Result<Duration, ConfigError> {
        self.number(name, default, 1..=600_000)
            .map(Duration::from_millis)
    }

    /// The value as a `usize` count of things a semaphore or a page holds.
    pub fn count(
        &self,
        name: &'static str,
        default: usize,
        max: usize,
    ) -> Result<usize, ConfigError> {
        self.number(name, default, 1..=max)
    }

    /// The value as a byte count, `1..=1 << 34` (16 GiB): past that a body cannot be read into a
    /// container that exists.
    ///
    /// Caveat: the ceiling is a guess from the container sizes in §6's memory budget, not a
    /// measurement; it exists so a typo cannot ask for a petabyte.
    pub fn bytes(&self, name: &'static str, default: u64) -> Result<u64, ConfigError> {
        self.number(name, default, 1..=1 << 34)
    }

    /// The bind address.
    pub fn bind(&self, default: IpAddr) -> Result<IpAddr, ConfigError> {
        self.parsed("GRAPH_HUB_BIND", default)
    }
}

fn refuse(name: &'static str, reason: &'static str) -> ConfigError {
    ConfigError::new(name, reason)
}

/// The byte caps and every duration, in §6's order. Each default is a literal.
pub(super) fn read_limits(env: &Env<'_>) -> Result<Limits, ConfigError> {
    Ok(Limits {
        max_body: capped(env.bytes("GRAPH_HUB_MAX_BODY", 4 << 20)?),
        max_batch: capped(env.number("GRAPH_HUB_MAX_BATCH", 10_000, 1..=1_000_000)?),
        max_record_bytes: capped(env.bytes("GRAPH_HUB_MAX_RECORD_BYTES", 1 << 20)?),
        max_plugin_bytes: capped(env.bytes("GRAPH_HUB_MAX_PLUGIN_BYTES", 16 << 20)?),
        max_doc_bytes: capped(env.bytes("GRAPH_HUB_MAX_DOC_BYTES", 64 << 20)?),
        retain: capped(env.number("GRAPH_HUB_RETAIN", 100_000, 1..=1_000_000_000)?),
        retain_bytes: capped(env.bytes("GRAPH_HUB_RETAIN_BYTES", 512 << 20)?),
        changes_bytes: capped(env.bytes("GRAPH_HUB_CHANGES_BYTES", 8 << 20)?),
        body_timeout: capped_millis(env.millis("GRAPH_HUB_BODY_TIMEOUT_MS", 10_000)?),
        stream_deadline: capped_millis(env.millis("GRAPH_HUB_STREAM_DEADLINE_MS", 120_000)?),
        motor_timeout: capped_millis(env.millis("GRAPH_HUB_MOTOR_TIMEOUT_MS", 45_000)?),
        timeout: capped_millis(env.millis("GRAPH_HUB_TIMEOUT_MS", 30_000)?),
        sse_page: capped(env.number("GRAPH_HUB_SSE_PAGE", 256, 1..=65_536)?),
        fetch_rows: capped(env.number("GRAPH_HUB_FETCH_ROWS", 4096, 1..=65_536)?),
        last_seen: capped(env.number("GRAPH_HUB_LAST_SEEN", 65_536, 1..=16_777_216)?),
    })
}

/// The connection limits, in §6's order.
pub(super) fn read_connections(env: &Env<'_>) -> Result<Connections, ConfigError> {
    Ok(Connections {
        max_connections: capped(env.count("GRAPH_HUB_MAX_CONNECTIONS", 256, 65_536)? as u64)
            as usize,
        header_timeout: capped_millis(env.millis("GRAPH_HUB_HEADER_TIMEOUT_MS", 5_000)?),
        // hyper refuses a read buffer under 8 KiB, so the cap is a range and not a `capped`.
        max_header_bytes: env.number("GRAPH_HUB_MAX_HEADER_BYTES", 16_384, 8_192..=1 << 20)?,
    })
}

/// The four semaphore sizes, in §6's order.
pub(super) fn read_gates(env: &Env<'_>) -> Result<Gates, ConfigError> {
    Ok(Gates {
        writers: capped(env.count("GRAPH_HUB_WRITERS", 2, 1024)? as u64) as usize,
        readers: capped(env.count("GRAPH_HUB_READS", 2, 1024)? as u64) as usize,
        layouts: capped(env.count("GRAPH_HUB_LAYOUTS", 1, 1024)? as u64) as usize,
        writers_per_key: capped(env.count("GRAPH_HUB_WRITERS_PER_KEY", 1, 1024)? as u64) as usize,
    })
}

/// The two subscriber caps, in §6's order.
pub(super) fn read_subscribers(env: &Env<'_>) -> Result<Subscribers, ConfigError> {
    Ok(Subscribers {
        max: capped(env.count("GRAPH_HUB_MAX_SUBSCRIBERS", 64, 65_536)? as u64) as usize,
        per_key: capped(env.count("GRAPH_HUB_MAX_SUBSCRIBERS_PER_KEY", 8, 65_536)? as u64) as usize,
    })
}

/// The store's own configuration, built from the reads already made plus the three variables only
/// the store names. The three it shares (`max_body`, `max_batch`, `max_record_bytes`) come from
/// [`Limits`], so a spec change that moves one moves both.
pub(super) fn read_store(env: &Env<'_>, limits: Limits) -> Result<StoreConfig, ConfigError> {
    let mut store = StoreConfig::defaults();
    store.pool = env.number("GRAPH_HUB_DB_POOL", 8, 1..=1024)?;
    store.max_body = limits.max_body;
    store.max_batch = limits.max_batch;
    store.max_record_bytes = limits.max_record_bytes;
    store.max_plugin_bytes = limits.max_plugin_bytes;
    store.max_doc_bytes = limits.max_doc_bytes;
    store.retain = limits.retain;
    store.retain_bytes = limits.retain_bytes;
    store.changes_bytes = limits.changes_bytes;
    store.fetch_rows = limits.fetch_rows;
    store.last_seen = limits.last_seen;
    store.timeout_ms = limits.timeout.as_millis() as u64;
    store.stream_deadline_ms = limits.stream_deadline.as_millis() as u64;
    Ok(store)
}

/// A variable's value as a path. An unset or empty value is an empty path, which the start check
/// refuses by name (Decision 7).
pub(super) fn path(env: &Env<'_>, name: &'static str) -> Result<PathBuf, ConfigError> {
    Ok(env.text(name)?.map_or_else(PathBuf::new, PathBuf::from))
}
