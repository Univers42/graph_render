//! The negative controls' knobs and the setting the native arm runs with: every
//! variable is read strictly and at most one may be set.

use super::knobs;

pub(super) mod arms;
pub(super) mod compute;
pub(crate) mod env;
pub(super) mod igraph;
pub(super) mod records;
pub(crate) mod setting;
pub(super) mod three_d;
pub(super) mod value;
mod wiring;
pub(crate) use setting::{Setting, env_setting};

mod kind;
pub(super) use arms::stage_of;
pub use kind::Knob;
