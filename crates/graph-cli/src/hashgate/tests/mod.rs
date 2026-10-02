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

use std::env::VarError;

/// A reader of the variables in `pairs`, every other one unset.
///
/// Takes an owned `Vec` rather than a `&'static` slice so a test can build its pairs from
/// a loop variable; a `'static` bound here would have forced every such test to spell out
/// a `const` table, which is noise around the claim being made.
pub(super) fn env(pairs: Vec<(&str, &str)>) -> impl Fn(&str) -> Result<String, VarError> {
    move |name| {
        let found = pairs.iter().find(|(key, _)| *key == name);
        found
            .map(|(_, value)| (*value).to_owned())
            .ok_or(VarError::NotPresent)
    }
}

/// The compiled-in defaults: no knob set, so a test that does not name one is measuring
/// the honest run.
pub(super) fn honest() -> Setting {
    setting(env(Vec::new())).expect("no knob set")
}
