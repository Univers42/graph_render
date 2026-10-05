//! The test-only seams: four hooks a test installs to force an interleaving or a pause that
//! timing alone would not produce.
//!
//! Without the `test-hooks` feature every function here is an empty (and where possible `const`)
//! function, so the optimizer removes the calls and a shipped binary carries no seam at all. Row
//! `hooks-gated-hub` proves both halves by reading `--print cfg` of a release build and of a test
//! build built with `--features test-hooks`.
//!
//! The signatures are the same in both builds: the [`Hooks`] value is a zero-sized type without the
//! feature, so a call site never has to know which build it is in.

#[cfg(feature = "test-hooks")]
use std::future::Future;
#[cfg(feature = "test-hooks")]
use std::pin::Pin;
#[cfg(feature = "test-hooks")]
use std::sync::Arc;

/// A hook's body: a future a test drives, boxed so the seam costs no generic plumbing at each call
/// site. `Send` because every seam runs on a request task.
#[cfg(feature = "test-hooks")]
pub type AsyncHook = Arc<dyn Fn() -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>;

/// The seams a test may install. Zero-sized, and empty in behaviour, without `test-hooks`.
#[cfg(feature = "test-hooks")]
#[derive(Clone, Default)]
pub struct Hooks {
    /// Runs after a route admits its permit and before it does anything else, so a test can hold
    /// one request inside a gate while another arrives.
    pub pause_after_admit: Option<AsyncHook>,
    /// Runs before a write's response acknowledges a `seq`, so a durability test can kill the
    /// process in the window between the store's commit and the acknowledgement.
    pub before_ack: Option<Arc<dyn Fn(u64) + Send + Sync>>,
    /// Runs with the number of change headers one stream read returned.
    pub count_headers: Option<Arc<dyn Fn(u64) + Send + Sync>>,
    /// Runs after a write read its body and before it parses it, so a memory test can put every
    /// writer's body in flight at once.
    pub hold_body: Option<AsyncHook>,
}

/// The seams. The whole type without `test-hooks`: there is nothing to install.
#[cfg(not(feature = "test-hooks"))]
#[derive(Clone, Copy, Debug, Default)]
pub struct Hooks;

#[cfg(not(feature = "test-hooks"))]
impl Hooks {
    /// Hooks with nothing installed.
    pub const fn new() -> Self {
        Self
    }

    /// Hooks with nothing installed: a shipped build reads no test variable.
    pub const fn from_env() -> Self {
        Self
    }
}

#[cfg(feature = "test-hooks")]
impl Hooks {
    /// Hooks with nothing installed.
    pub const fn new() -> Self {
        Self {
            pause_after_admit: None,
            before_ack: None,
            count_headers: None,
            hold_body: None,
        }
    }

    /// The hooks the environment asks for: `GM_HUB_HOLD_BODIES=N` makes [`Hooks::hold_body`] a
    /// barrier N writes reach before any of them parses. Row `hub-memory` measures a hub running as
    /// its own process, where a test cannot install a closure, so the variable is the only way in.
    ///
    /// Caveat: the barrier waits for exactly N writes. A run that sends fewer leaves the ones it
    /// sent parked until their client gives up, so the caller must send N, and send them at once.
    pub fn from_env() -> Self {
        let mut hooks = Self::new();
        let parties = std::env::var("GM_HUB_HOLD_BODIES").ok();
        if let Some(parties) = parties.and_then(|n| n.parse::<usize>().ok()) {
            let barrier = Arc::new(tokio::sync::Barrier::new(parties));
            hooks.hold_body = Some(Arc::new(move || {
                let barrier = Arc::clone(&barrier);
                Box::pin(async move {
                    barrier.wait().await;
                })
            }));
        }
        hooks
    }
}

/// Pause after a route admitted its permit, naming the route so one hook can tell them apart.
#[cfg(feature = "test-hooks")]
pub async fn pause_after_admit(hooks: &Hooks, route: &'static str) {
    if let Some(hook) = &hooks.pause_after_admit {
        hook().await;
    }
    let _ = route;
}

/// Pause after a route admitted its permit. A no-op without `test-hooks`, so the optimizer removes
/// the state machine in a shipped build and the call site folds away.
#[cfg(not(feature = "test-hooks"))]
pub async fn pause_after_admit(_hooks: &Hooks, _route: &'static str) {}

/// Hold a write between reading its body and parsing it.
#[cfg(feature = "test-hooks")]
pub async fn hold_body(hooks: &Hooks) {
    if let Some(hook) = &hooks.hold_body {
        hook().await;
    }
}

/// Hold a write between reading its body and parsing it. A no-op without `test-hooks`.
#[cfg(not(feature = "test-hooks"))]
pub async fn hold_body(_hooks: &Hooks) {}

/// Run the acknowledgement seam for `seq`, before a write answers.
#[cfg(feature = "test-hooks")]
pub fn before_ack(hooks: &Hooks, seq: u64) {
    if let Some(hook) = &hooks.before_ack {
        hook(seq);
    }
}

/// Run the acknowledgement seam for `seq`. A no-op without `test-hooks`.
#[cfg(not(feature = "test-hooks"))]
pub const fn before_ack(_hooks: &Hooks, _seq: u64) {}

/// Run the header-counting seam with `count`, the headers one stream read returned.
#[cfg(feature = "test-hooks")]
pub fn count_headers(hooks: &Hooks, count: u64) {
    if let Some(hook) = &hooks.count_headers {
        hook(count);
    }
}

/// Run the header-counting seam with `count`. A no-op without `test-hooks`.
#[cfg(not(feature = "test-hooks"))]
pub const fn count_headers(_hooks: &Hooks, _count: u64) {}
