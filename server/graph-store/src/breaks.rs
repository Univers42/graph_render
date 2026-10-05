//! The server-side negative controls, behind the off-by-default `negctl` feature.
//!
//! Each name turns exactly one claim in the contract red, so a green control proves the row can
//! fail. `GM_HUB_BREAK` is a comma-separated list; an unset variable means no break is on.
//!
//! Nothing in this module is reachable from a shipped build: without `negctl` [`on`] is a
//! `const fn` returning `false`, which the optimizer removes.

/// The breaks this slice's rows force.
pub const NAMES: [&str; 13] = [
    "sequence-seq",
    "no-idem",
    "changes-read-committed",
    "sync-commit-unset",
    "prune-own-transaction",
    "no-trigger",
    "trigger-enable-origin",
    "one-trigger-origin",
    "detector-at-start",
    "checkpoint-timeline",
    "lsn-only",
    "hw-after-lsn",
    "no-deadlock-retry",
];

/// Is the named break on?
#[cfg(feature = "negctl")]
pub fn on(name: &str) -> bool {
    let var = std::env::var("GM_HUB_BREAK").unwrap_or_default();
    var.split(',').any(|b| b.trim() == name)
}

/// Is the named break on? Always `false`, and `const`, so a build without `negctl` carries no
/// seam at all.
#[cfg(not(feature = "negctl"))]
pub const fn on(_name: &str) -> bool {
    false
}