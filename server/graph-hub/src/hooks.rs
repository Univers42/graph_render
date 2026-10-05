//! The test-only seams: four hooks a test installs to force an interleaving, a pause or a fault that
//! timing alone would not produce.
//!
//! Without the `test-hooks` feature every function here is an empty (and where possible `const`)
//! function, so the optimizer removes the calls and a shipped binary carries no seam at all. Row
//! `hooks-gated-hub` proves both halves by reading `--print cfg` of a release build and of a test
//! build built with `--features test-hooks`.
//!
//! The signatures are the same in both builds: the [`Hooks`] value is a zero-sized type without the
//! feature, so a call site never has to know which build it is in.
//!
//! WHY each installed hook sits behind an `Arc<Mutex<Option<…>>>` rather than a plain `Option`: the
//! seams live in an [`crate::app::App`] that a test holds as an `Arc`, and a fixture can only install
//! one after the router is built. Interior mutability is what makes "install after the router
//! exists" possible without `App` being mutable everywhere. Each seam locks, copies the `Arc` out and
//! drops the guard before it awaits, so no lock is ever held across an `.await` and no seam can
//! deadlock another.

#[cfg(feature = "test-hooks")]
use std::future::Future;
#[cfg(feature = "test-hooks")]
use std::pin::Pin;
#[cfg(feature = "test-hooks")]
use std::sync::{Arc, Mutex};

/// A hook's body: a future a test drives, boxed so the seam costs no generic plumbing at each call
/// site. `Send` because every seam runs on a request task.
///
/// The `&'static str` is the route's own name, which is what lets one hook tell two routes apart: a
/// case that must prove a reconnect read no `/graph` needs to count `/graph` and nothing else.
#[cfg(feature = "test-hooks")]
pub type AsyncHook =
    Arc<dyn Fn(&'static str) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>;

/// A counting hook's body: the number it is handed, and nothing else.
#[cfg(feature = "test-hooks")]
pub type CountHook = Arc<dyn Fn(u64) + Send + Sync>;

/// A fault hook's body: the store fault a read after `since` should meet, if any.
#[cfg(feature = "test-hooks")]
pub type FaultHook = Arc<dyn Fn(u64) -> Option<graph_store::StoreError> + Send + Sync>;

/// A slot a test writes once and every call site reads, which is the interior mutability above.
#[cfg(feature = "test-hooks")]
pub type Slot<T> = Arc<Mutex<Option<T>>>;

/// The seams a test may install. Zero-sized, and empty in behaviour, without `test-hooks`.
#[cfg(feature = "test-hooks")]
#[derive(Clone)]
pub struct Hooks {
    /// Runs after a route admits its permit and before it does anything else, so a test can hold
    /// one request inside a gate while another arrives.
    pub pause_after_admit: Slot<AsyncHook>,
    /// Runs before a write's response acknowledges a `seq`, so a durability test can kill the
    /// process in the window between the store's commit and the acknowledgement.
    pub before_ack: Slot<CountHook>,
    /// Runs with the number of change headers one stream read returned.
    pub count_headers: Slot<CountHook>,
    /// Answers the store fault a stream read should meet, given the cursor it read after.
    ///
    /// WHY a function and not a flag: a stream's two early closes are decided by *which* fault the
    /// read meets — `Gone` is a `resync`, `Busy` is a `busy` — and a flag could force only one of
    /// them. Row `the_busy_slot_is_free_before_the_close` needs `Busy` on demand, and a pool cannot
    /// be made short on demand.
    pub page_fault: Slot<FaultHook>,
}

#[cfg(feature = "test-hooks")]
impl std::fmt::Debug for Hooks {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Hooks").finish_non_exhaustive()
    }
}

#[cfg(feature = "test-hooks")]
impl Hooks {
    /// Hooks with nothing installed.
    pub fn new() -> Self {
        Hooks {
            pause_after_admit: Slot::default(),
            before_ack: Slot::default(),
            count_headers: Slot::default(),
            page_fault: Slot::default(),
        }
    }

    /// Run `hook` with `value`, or with nothing when the slot is empty.
    ///
    /// The slot is locked, the `Arc` copied out and the guard dropped before the call, so a hook
    /// that itself reads a seam cannot deadlock on this one.
    fn run<T: Clone>(slot: &Slot<T>, call: impl FnOnce(T)) {
        let hook = slot.lock().ok().and_then(|held| held.clone());
        if let Some(hook) = hook {
            call(hook);
        }
    }
}

#[cfg(not(feature = "test-hooks"))]
/// The seams. The whole type without `test-hooks`: there is nothing to install.
#[derive(Clone, Copy, Debug, Default)]
pub struct Hooks;

#[cfg(not(feature = "test-hooks"))]
impl Hooks {
    /// Hooks with nothing installed.
    pub const fn new() -> Self {
        Self
    }
}

/// Pause after a route admitted its permit, naming the route so one hook can tell them apart.
#[cfg(feature = "test-hooks")]
pub async fn pause_after_admit(hooks: &Hooks, route: &'static str) {
    let hook = hooks
        .pause_after_admit
        .lock()
        .ok()
        .and_then(|held| held.clone());
    if let Some(hook) = hook {
        hook(route).await;
    }
}

/// Pause after a route admitted its permit. A no-op without `test-hooks`, so the optimizer removes
/// the state machine in a shipped build and the call site folds away.
#[cfg(not(feature = "test-hooks"))]
pub async fn pause_after_admit(_hooks: &Hooks, _route: &'static str) {}

/// Run the acknowledgement seam for `seq`, before a write answers.
#[cfg(feature = "test-hooks")]
pub fn before_ack(hooks: &Hooks, seq: u64) {
    Hooks::run(&hooks.before_ack, |hook| hook(seq));
}

/// Run the acknowledgement seam for `seq`. A no-op without `test-hooks`.
#[cfg(not(feature = "test-hooks"))]
pub const fn before_ack(_hooks: &Hooks, _seq: u64) {}

/// Run the header-counting seam with `count`, the headers one stream read returned.
#[cfg(feature = "test-hooks")]
pub fn count_headers(hooks: &Hooks, count: u64) {
    Hooks::run(&hooks.count_headers, |hook| hook(count));
}

/// Run the header-counting seam with `count`. A no-op without `test-hooks`.
#[cfg(not(feature = "test-hooks"))]
pub const fn count_headers(_hooks: &Hooks, _count: u64) {}

/// The fault a stream read after `since` should meet, or `None` to read for real.
#[cfg(feature = "test-hooks")]
pub fn page_fault(hooks: &Hooks, since: u64) -> Option<graph_store::StoreError> {
    let hook = hooks.page_fault.lock().ok().and_then(|held| held.clone());
    match hook {
        Some(hook) => hook(since),
        None => None,
    }
}

/// The fault seam. Always `None`, so a shipped build reads the database and nothing else.
#[cfg(not(feature = "test-hooks"))]
pub const fn page_fault(_hooks: &Hooks, _since: u64) -> Option<graph_store::StoreError> {
    None
}
