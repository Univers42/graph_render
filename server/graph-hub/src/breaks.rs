//! The server-side negative controls, behind the off-by-default `negctl` feature.
//!
//! Each name turns exactly one claim of the contract red, so a green control proves the row it
//! belongs to can fail. `GM_HUB_BREAK` is a comma-separated list; an unset variable means no break
//! is on. graph-store's and graph-contract's breaks are read through the same variable and are
//! named in their own crates' tables (`server/graph-store/src/breaks.rs`,
//! `crates/graph-contract/src/hub/breaks.rs`).
//!
//! Nothing here is reachable from a shipped build: without `negctl`, [`on`] is a `const fn`
//! returning `false`, which the optimizer removes. Row `hooks-gated-hub` reads `--print cfg` to
//! prove the release build carries no `negctl` and no `test-hooks`.

/// The breaks this crate's own rows force. Task 2 adds `no-start-check`, Task 3 `skip-grant` and
/// `reload-keys-only`, Task 4 `no-cap`, Task 7 `skip-event`, Task 8 `drop-record`.
pub const NAMES: [&str; 5] = [
    "skip-grant",
    "reload-keys-only",
    "no-cap",
    "no-start-check",
    "skip-event",
];

/// Is the named break on?
///
/// The `trim` is there because a row passes one name per variable; a break list with a space after
/// the comma is the same list.
#[cfg(feature = "negctl")]
pub fn on(name: &str) -> bool {
    let var = std::env::var("GM_HUB_BREAK").unwrap_or_default();
    var.split(',').any(|b| b.trim() == name)
}

/// Is the named break on? Always `false`, and `const`, so a build without `negctl` carries no seam.
#[cfg(not(feature = "negctl"))]
pub const fn on(_name: &str) -> bool {
    false
}
