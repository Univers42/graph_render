//! The negative controls' server-side breaks (`scripts/orch/rows/service.rows`). Each names one
//! property a row checks and turns it off, so the row must go red. They exist only under the
//! `negctl` feature: a shipped build never reads `GM_SVC_BREAK`.

/// True when the break `name` is on: `GM_SVC_BREAK` is a comma list of break names.
#[cfg(feature = "negctl")]
pub fn on(name: &str) -> bool {
    std::env::var("GM_SVC_BREAK").is_ok_and(|list| list.split(',').any(|item| item == name))
}

/// Always false without the `negctl` feature.
#[cfg(not(feature = "negctl"))]
pub const fn on(_name: &str) -> bool {
    false
}
