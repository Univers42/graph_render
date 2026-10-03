//! The honest run: the compiled-in defaults, and the one comparison that decides whether a
//! setting perturbs anything (RG-42). A child of `setting.rs`, split out by the house's
//! 300-line limit.

use super::Setting;
use graph_core::layout::circle_packing::CirclePackingParams;
use graph_core::layout::force::spring::SpringParams;
use graph_core::layout::force::{ForceParams, LiveParams, Split};
use graph_core::layout::forceatlas2::Fa2Params;
use graph_core::layout::graphviz::neato;
use graph_core::post::separate::SeparateParams;
use graph_core::{GridParams, REFERENCE_DEGREE, SugiyamaParams};

impl Setting {
    /// The compiled-in defaults: the honest run, with no knob set.
    ///
    /// Named, and not a `Default` impl, because "the honest run" is the value
    /// [`Setting::bites`] compares every control against — it is a second definition of what
    /// the gate runs when nothing perturbs it, and there is one of them.
    pub(crate) fn compiled_in() -> Setting {
        Setting {
            reference_degree: REFERENCE_DEGREE,
            grid: GridParams::default(),
            sugiyama: SugiyamaParams::default(),
            extra_nodes: 0,
            force: ForceParams::default(),
            fa2: Fa2Params::default(),
            spring: SpringParams::default(),
            packing: CirclePackingParams::default(),
            neato_epsilon: None,
            stage_nodes: None,
            overlap_relaxation: None,
            split_sum: Split::None,
            split_rescale: false,
            live_gravity: None,
            layout_param_default: None,
            control: None,
        }
    }

    /// **Whether this run perturbs anything at all** (RG-42) — one comparison against the
    /// compiled-in defaults, so every knob is covered by the rule rather than by an arm
    /// remembering to apply it.
    ///
    /// `control` is left out: it names *which* knob was set, not what the run computes. The
    /// two `Option` fields are normalised through their accessors first, because `neato`'s
    /// `EPSILON` and the live session's `0` gravity are the honest values spelled out
    /// explicitly — a flag that could not say "the default" would make those two controls
    /// inexpressible, and a field comparison alone would call them perturbations.
    pub(crate) fn bites(&self) -> bool {
        self.normalised() != Self::compiled_in().normalised()
    }

    /// `self` with every field that is only an `Option` *because* it has to be able to say
    /// "unset" collapsed to unset, and `control` cleared.
    fn normalised(self) -> Setting {
        let mut out = self;
        if out.neato_epsilon() == neato::EPSILON {
            out.neato_epsilon = None;
        }
        if out.live_force_params().gravity == LiveParams::default().gravity {
            out.live_gravity = None;
        }
        let default_relaxation = SeparateParams::default().over_relaxation;
        if out.overlap_relaxation.map(|r| r as f32) == Some(default_relaxation) {
            out.overlap_relaxation = None;
        }
        out.control = None;
        out
    }
}
