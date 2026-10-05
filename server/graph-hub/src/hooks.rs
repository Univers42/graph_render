//! The test-only seams: three hooks a test installs to force an interleaving or a pause that
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
}

#[cfg(feature = "test-hooks")]
impl Hooks {
    /// Hooks with nothing installed.
    pub const fn new() -> Self {
        Self {
            pause_after_admit: None,
            before_ack: None,
            count_headers: None,
        }
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
