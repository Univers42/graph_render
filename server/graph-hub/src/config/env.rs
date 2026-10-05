//! Every environment variable the hub reads, in the order the start line lists them, and the
//! reader that turns one into a typed value.
//!
//! An empty variable counts as unset, exactly as graph-server's reader does
//! (`server/graph-server/src/config.rs:253-262`), so `GRAPH_HUB_MAX_BODY=` is the default and not
//! a malformed value. A refused value names the variable and never its value: `GRAPH_HUB_DB_URL`
//! and `GRAPH_HUB_MOTOR_KEY_FILE` name a secret's location.

use std::ffi::OsString;
use std::net::IpAddr;
use std::str::FromStr;
use std::time::Duration;

/// Every variable the hub reads, in the order the start line lists them.
///
/// The first five are the ones §6's table does not bound (where the listener binds, where the
/// database is, and the two credential files H8 names); the rest are §6's twenty-six.
///
/// `GRAPH_HUB_MOTOR_URL` and `GRAPH_HUB_MOTOR_KEY_FILE` are this slice's names for §6's
/// `GRAPH_MOTOR_URL` and `GRAPH_MOTOR_KEY_FILE`: every other name in that table carries the
/// `GRAPH_HUB_` prefix, the motor key is the hub's own credential (Decision 6), and the plan's
/// `hub-upload-timeout` row already passes `GRAPH_HUB_MOTOR_URL`.
pub const NAMES: [&str; 31] = [
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
