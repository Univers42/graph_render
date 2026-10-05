//! The negative controls' breaks (`scripts/orch/rows/hub-contract.rows`). Each names one
//! property a row checks and turns it off, so the row must go red.
//!
//! A copy of `server/graph-server/src/breaks.rs` with the variable renamed, because the
//! two must be read independently: a shared module would mean the service's negative
//! controls and this crate's could not be run in one environment without one build
//! turning the other's off.
//!
//! They exist only under the `negctl` feature: a shipped build never reads
//! `GM_HUB_BREAK`, and the non-`negctl` `on` is a `const fn` returning `false` so the
//! check folds away entirely rather than reading an environment at every cell check.
//!
//! | Break | Turns off | Row that must go red |
//! |---|---|---|
//! | `lax-reader` | the NUL walk and the `B.coll` collection-id check | `negctl-lax-reader` |
//! | `keep-dangling` | pruning a record cell whose target does not exist | `negctl-keep-dangling` |
//! | `keep-cells` | pruning the *cells* of a dropped link field | `negctl-keep-cells` |

/// True when the break `name` is on: `GM_HUB_BREAK` is a comma list of break names.
#[cfg(feature = "negctl")]
pub fn on(name: &str) -> bool {
    std::env::var("GM_HUB_BREAK").is_ok_and(|list| list.split(',').any(|item| item == name))
}

/// Always false without the `negctl` feature.
#[cfg(not(feature = "negctl"))]
pub const fn on(_name: &str) -> bool {
    false
}
