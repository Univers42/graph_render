//! The gate's test module, one child per claim so each stays inside the 300-line cap:
//!
//! - [`compare`] — the comparison itself: N-way `diverged`, the per-stage tally, and the
//!   refusals for an input that *cannot* fail.
//! - [`pipeline`] — what the arms run: the stage list, the registered pipeline's bytes,
//!   and which knob moves which stage.
//! - [`arm_lines`] — what an arm *prints*: the line order, and the threaded arm's claim to
//!   be the scalar arm's lines verbatim.
//! - [`stage_list`] — the stage list as a list: no id twice across all three registries.
//! - [`knob`], [`report`], [`stages`] — the settings, the written report and the stage
//!   bytes, each with its own claims.

mod arm_lines;
mod compare;
mod knob;
mod pipeline;
mod report;
mod stage_list;
mod stages;

// The vocabulary the *direct* children name from here. A grandchild (`tests::stages::arm`)
// reaches the gate's own items through `super::super::super::` instead, because this file
// is one level nearer the gate than it is. Named rather than globbed, so a test file says
// where each name comes from instead of inheriting whatever the parent happened to import.
use super::compare::{Arm, Tally};
use super::knob::Knob;
use super::knob::setting::{Setting, setting};
use super::stage_bytes;
use super::tier;

use super::knob::Env;

/// The variables in `pairs` and no others, as the environment `setting` reads.
///
/// Both questions the reader asks — the value of a name, and which names are there — come
/// from the one value, so a test that sets a knob cannot leave the `GM_MUTATE_*` typo sweep
/// (RG-26) looking at the real process environment.
pub(super) fn env(pairs: Vec<(&str, &str)>) -> Env {
    Env::list(pairs)
}

/// The compiled-in defaults: no knob set, so a test that does not name one is measuring
/// the honest run.
pub(super) fn honest() -> Setting {
    setting(env(Vec::new())).expect("no knob set")
}
