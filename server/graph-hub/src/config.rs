//! §6's settings, read from the environment only, and the start checks that refuse a hub whose
//! deployment cannot honour the contract.
//!
//! Every default below is spelled out as a literal in [`Settings::from_env`], so a spec change that
//! renames or moves one fails `config_defaults_match_section_6` rather than silently changing a
//! behaviour. Every one of them carries its own `Caveat:` line, and each is a number an operator
//! may want to move, so none of them is a measurement.
//!
//! The start checks are [`Settings::check`]: §6's five refusals that need no connection, then the
//! two that need one. They run **before** the listener is bound, so a hub that fails a check never
//! answers a request.

mod check;
mod env;

pub use check::{check_database, migrate_database};
pub use env::{ConfigError, Env, Lookup, NAMES};

use crate::breaks;
use env::{read_connections, read_gates, read_limits, read_store, read_subscribers};
use graph_store::StoreConfig;
use std::net::IpAddr;
use std::path::PathBuf;
use std::time::Duration;

/// The byte and count caps, and every duration, in one copy-able struct.
///
/// Caveat on the shape: `max_body`, `max_batch` and `max_record_bytes` are also the three fields of
/// `graph_contract::hub::Limits`, and `StoreConfig` holds them again. They are one number read
/// once and copied twice rather than a single source, because the contract's `Limits` is a wire
/// type and this one is a deployment; [`Settings::from_env`] builds all three from the same read,
/// so a spec change that moves one moves all three.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// `GRAPH_HUB_MAX_BODY`: bytes in one request body, chunked bodies included. Default 4 MiB.
    /// Caveat: 4 MiB is graph-contract's `Limits::DEFAULT::max_body`, so a body the hub accepts is
    /// one the wire admits; it is not a memory bound, and `hub-memory` measures what it costs.
    pub max_body: u64,
    /// `GRAPH_HUB_MAX_BATCH`: operations in one batch. Default 10 000.
    /// Caveat: 10 000 is the contract's own `max_batch`; past it a batch is refused whole rather
    /// than split, so one oversized batch is the caller's problem to divide.
    pub max_batch: u64,
    /// `GRAPH_HUB_MAX_RECORD_BYTES`: bytes in one record's canonical text. Default 1 MiB.
    /// Caveat: 1 MiB of *canonical text*, so a record whose JSON is twice that size is under the
    /// cap; the writer compares canonical bytes, and this is the number the reader enforces.
    pub max_record_bytes: u64,
    /// `GRAPH_HUB_MAX_PLUGIN_BYTES`: bytes one plugin's records may reach. Default 16 MiB.
    /// Caveat: 16 MiB per plugin against a 64 MiB workspace, so four plugins can fill a workspace
    /// and a fifth is refused for the workspace's cap, learning only that the workspace is full.
    pub max_plugin_bytes: u64,
    /// `GRAPH_HUB_MAX_DOC_BYTES`: bytes one workspace's document may reach. Default 64 MiB.
    /// Caveat: 64 MiB equals graph-server's own `GRAPH_MAX_BODY` default, so a workspace the hub
    /// accepts fits the motor's body limit; the motor's per-id caps can still answer 413 (§5.2).
    pub max_doc_bytes: u64,
    /// `GRAPH_HUB_RETAIN`: changes kept per workspace. Default 100 000.
    /// Caveat: 100 000 changes is a bound on a *cursor's* age, not on bytes, so a workspace of
    /// large changes prunes sooner than one of small ones.
    pub retain: u64,
    /// `GRAPH_HUB_RETAIN_BYTES`: bytes of change log kept per workspace. Default 512 MiB.
    /// Caveat: 512 MiB per workspace, so retention is unbounded in aggregate across workspaces;
    /// §6's memory budget does not carry it either.
    pub retain_bytes: u64,
    /// `GRAPH_HUB_CHANGES_BYTES`: bytes one `/changes` page may carry. Default 8 MiB.
    /// Caveat: 8 MiB bounds a page, and the start check refuses anything below one maximum
    /// change, so a page always holds at least one change rather than being unable to.
    pub changes_bytes: u64,
    /// `GRAPH_HUB_BODY_TIMEOUT_MS`: reading a whole request body. Default 10 000 ms.
    /// Caveat: 10 000 ms is the hub's own budget and graph-server enforces an identical 10 000 on
    /// its side of the relay, independently. Nothing orders the two.
    pub body_timeout: Duration,
    /// `GRAPH_HUB_STREAM_DEADLINE_MS`: `/graph` and `/layout` bodies. Default 120 000 ms.
    /// Caveat: 120 000 ms bounds a slow client, and a client cut here sees a short body and
    /// retries; it is not a rate limit and does not stop a client that reconnects at once.
    pub stream_deadline: Duration,
    /// `GRAPH_HUB_MOTOR_TIMEOUT_MS`: the whole `/layout` relay. Default 45 000 ms.
    /// Caveat: 45 000 ms is graph-server's `GRAPH_TIMEOUT_MS` plus its `GRAPH_BODY_TIMEOUT_MS`
    /// (30 000 + 10 000), so the motor answers first; the start check refuses 40 000 or below.
    pub motor_timeout: Duration,
    /// `GRAPH_HUB_TIMEOUT_MS`: a permit or pool wait, and the shutdown drain. Default 30 000 ms.
    /// Caveat: 30 000 ms bounds the *wait* and never the work, so a slow database still holds a
    /// permit and a pool connection for as long as it takes.
    pub timeout: Duration,
    /// `GRAPH_HUB_SSE_PAGE`: change headers per stream read. Default 256.
    /// Caveat: 256 headers per read bounds one page, and the next read continues from the last seq
    /// sent; a subscriber past the cap is not slowed, it is served in more pages.
    pub sse_page: u64,
    /// `GRAPH_HUB_FETCH_ROWS`: rows one document page reads at most. Default 4096.
    /// Caveat: 32 rows is a guess about a record's size, so a workspace of large records reads the
    /// same number of bytes per round trip as a workspace of small ones reads many times more.
    pub fetch_rows: u64,
    /// `GRAPH_HUB_LAST_SEEN`: entries in the detector's last-seen map. Default 65 536.
    /// Caveat: 65 536 entries is the map the restore detector compares against (§5.3); the
    /// least recently used entry is evicted, and a workspace evicted past it is no longer covered
    /// by the last-seen half of the detector.
    pub last_seen: u64,
}

/// The connection limits hyper enforces before a request reaches the router.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Connections {
    /// `GRAPH_HUB_MAX_CONNECTIONS`: open connections. Default 256.
    /// Caveat: 256 permits are held from before `accept`, so past the cap a connection waits in the
    /// kernel backlog and the client, not the hub, gives up. There is no rejection response and no
    /// refusal log line for the overflow.
    pub max_connections: usize,
    /// `GRAPH_HUB_HEADER_TIMEOUT_MS`: receiving the request head. Default 5 000 ms.
    /// Caveat: a read timeout per read, not a bound on the head as a whole, so a client that
    /// dribbles one byte per read is cut by the interval alone.
    pub header_timeout: Duration,
    /// `GRAPH_HUB_MAX_HEADER_BYTES`: hyper's read buffer. Default 16 384.
    /// Caveat: 16 384 cannot go below 8 192, hyper's own floor, and past it hyper answers a bare
    /// 431 — not the JSON error body — so a request that large never reaches a log line.
    pub max_header_bytes: usize,
}

/// The four semaphores: every route takes exactly one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Gates {
    /// `GRAPH_HUB_WRITERS`: batches, manifest PUTs and workspace creates at once. Default 2.
    /// Caveat: 2 concurrent writers against `GRAPH_HUB_DB_POOL` = 8 leaves the pool six
    /// connections, and the start check refuses a pool that does not cover the permits at all.
    pub writers: usize,
    /// `GRAPH_HUB_READS`: `/graph`, `/changes`, records pages, `GET /plugins`, one record and
    /// `/workspaces` at once. Default 2.
    /// Caveat: a READS permit bounds concurrent materializations and not the bytes one of them
    /// streams; `GRAPH_HUB_STREAM_DEADLINE_MS` is what cuts that.
    pub readers: usize,
    /// `GRAPH_HUB_LAYOUTS`: `/layout` at once. Default 1.
    /// Caveat: 1, because graph-server admits a request before it reads the body
    /// (`server/graph-server/src/layout.rs:64-65`), so a waiting `/layout` holds a snapshot, an
    /// xmin horizon and a pool connection for up to the motor's 30 s.
    pub layouts: usize,
    /// `GRAPH_HUB_WRITERS_PER_KEY`: the WRITERS permits one key holds at once. Default 1.
    /// Caveat: 1 serializes one key's writes; it is a per-key semaphore over a map, so a key that
    /// never writes again has its entry dropped rather than kept for the hub's life.
    pub writers_per_key: usize,
}

/// The two subscriber counters of §6's `MAX_SUBSCRIBERS` pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Subscribers {
    /// `GRAPH_HUB_MAX_SUBSCRIBERS`: streams at once. Default 64.
    /// Caveat: 64 is the count of streams, not of subscribers: one client may hold several, and
    /// each is a task plus the workspace's watch receiver.
    pub max: usize,
    /// `GRAPH_HUB_MAX_SUBSCRIBERS_PER_KEY`: streams one key holds. Default 8.
    /// Caveat: 8 per key against 64 in total, so a key can be refused at 8 while the hub has room
    /// for 56 more from other keys.
    pub per_key: usize,
}

/// Everything the hub is configured with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    /// `GRAPH_HUB_BIND`: the listen address. Loopback by default.
    pub bind: IpAddr,
    /// `GRAPH_HUB_PORT`; 0 asks the kernel for a free port.
    pub port: u16,
    /// `GRAPH_HUB_DB_URL`: the `postgres://` URL, including its role and password. It is a secret
    /// and never appears in a log line or a start refusal.
    pub db_url: String,
    /// `GRAPH_HUB_KEYS_FILE`: the key file, `<name> <sha256-hex>` lines, 0640 or stricter.
    pub keys_file: PathBuf,
    /// `GRAPH_HUB_GRANTS_FILE`: the grants file, one grant per line, 0640 or stricter.
    pub grants_file: PathBuf,
    /// `GRAPH_HUB_MOTOR_URL`: graph-server's own base URL. Default `http://127.0.0.1:8080`.
    ///
    /// Caveat: loopback by default, because the motor and the hub are two containers on one host
    /// and only `scripts/orch/hub-run.sh` knows the bridge IP; a real deployment sets it.
    pub motor_url: String,
    /// `GRAPH_HUB_MOTOR_KEY_FILE`: the file whose first line is the plaintext motor key.
    ///
    /// Caveat: a live credential, mode 0600. It is a file and not the key itself because a
    /// graph-server keys file stores only `name <sha256-hex>`, so the secret cannot be recovered
    /// from one (Decision 6).
    pub motor_key_file: PathBuf,
    /// The store's own configuration, built from the same reads as [`Limits`].
    pub store: StoreConfig,
    /// The byte, count and duration caps.
    pub limits: Limits,
    /// The connection limits.
    pub connections: Connections,
    /// The four semaphores' sizes.
    pub gates: Gates,
    /// The two subscriber caps.
    pub subscribers: Subscribers,
    /// `GRAPH_HUB_SSE_PAGE`, lifted out of [`Limits`] because the stream reads it by name.
    pub sse_page: u64,
    /// `GRAPH_HUB_MOTOR_TIMEOUT_MS`, lifted out for the same reason.
    pub motor_timeout: Duration,
    /// `GRAPH_HUB_TIMEOUT_MS`, lifted out for the same reason.
    pub timeout: Duration,
}

/// The `no-cap` break's answer for one number: `u64::MAX`, so every cap in the hub disappears at
/// once and the 413, the 429, the 408 and both 503s go with it.
///
/// Why one function rather than a check per reader: row `negctl-no-cap` has to turn *every* limit
/// red, and a reader that forgot the check would leave one cap standing while the control claims all
/// of them went. The check is therefore in the one place every number passes through.
///
/// Caveat: `u64::MAX` is a fiction rather than a number the hub could use — a body that large cannot
/// be read into a container that exists — which is exactly what the control needs and exactly what no
/// shipped build can reach.
pub fn capped(value: u64) -> u64 {
    if breaks::on("no-cap") {
        u64::MAX
    } else {
        value
    }
}

/// [`capped`] for a duration: long enough that no test's budget is out of it.
pub fn capped_millis(value: Duration) -> Duration {
    if breaks::on("no-cap") {
        Duration::from_millis(u64::MAX / 2)
    } else {
        value
    }
}

/// `GRAPH_HUB_PORT` alone, for `healthcheck`: the image has no `curl`, so the binary probes its own
/// `/healthz` (`server/graph-server/src/config.rs:120-122` does the same).
pub fn port(lookup: Lookup<'_>) -> Result<u16, ConfigError> {
    Env(lookup).number("GRAPH_HUB_PORT", 8080, 0..=u16::MAX)
}

impl Settings {
    /// Reads and checks every variable, in one pass and with every §6 default as a literal.
    ///
    /// The store's [`StoreConfig`] is built from the same reads, so there is one number per
    /// variable and not two that can disagree.
    pub fn from_env(lookup: Lookup<'_>) -> Result<Self, ConfigError> {
        let env = Env(lookup);
        let limits = read_limits(&env)?;
        let store = read_store(&env, limits)?;
        Ok(Settings {
            bind: env.bind(IpAddr::from([127, 0, 0, 1]))?,
            port: env.number("GRAPH_HUB_PORT", 8080, 0..=u16::MAX)?,
            db_url: env.text("GRAPH_HUB_DB_URL")?.unwrap_or_default(),
            keys_file: env::path(&env, "GRAPH_HUB_KEYS_FILE")?,
            grants_file: env::path(&env, "GRAPH_HUB_GRANTS_FILE")?,
            motor_url: env
                .text("GRAPH_HUB_MOTOR_URL")?
                .unwrap_or_else(|| String::from("http://127.0.0.1:8080")),
            motor_key_file: env::path(&env, "GRAPH_HUB_MOTOR_KEY_FILE")?,
            store,
            limits,
            connections: read_connections(&env)?,
            gates: read_gates(&env)?,
            subscribers: read_subscribers(&env)?,
            sse_page: limits.sse_page,
            motor_timeout: limits.motor_timeout,
            timeout: limits.timeout,
        })
    }

    /// The start line: every variable's name with `set` or `unset`, never a value. `docs/deploy/hub.md`
    /// says the same, and it is why the line is safe to log in full.
    pub fn start_line(lookup: Lookup<'_>) -> String {
        let mut env = serde_json::Map::new();
        for name in NAMES {
            let set = lookup(name).is_some_and(|value| !value.is_empty());
            env.insert(name.to_owned(), (if set { "set" } else { "unset" }).into());
        }
        let version = env!("CARGO_PKG_VERSION");
        serde_json::json!({ "event": "start", "version": version, "env": env }).to_string()
    }
}
