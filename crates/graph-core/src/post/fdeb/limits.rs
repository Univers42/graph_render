//! The ceilings `check` refuses past, one per knob the reference panel bounds with a
//! `max=`. A caller cannot buy work the reference's UI would not sell either.
//!
//! They are a child module because `fdeb.rs` was already at the house's 300-line cap; the
//! module root re-exports all three, so `post::fdeb::MAX_*` is where they have always been.

/// The most subdivision points per edge the pass accepts: the reference panel's own
/// `edge_segments` max (`SciGraphs/properties/edge_style_properties.py:78-85`).
///
/// Ponytail: the reference UI's ceiling, not a measurement. A caller asking for a finer row
/// is refused although the arithmetic would hold well past it; what it buys is that
/// `segments + 2` and the row buffers can never overflow. Escape hatch: raise the const.
pub const MAX_SEGMENTS: u32 = 32;

/// The most schedule cycles the pass accepts: the reference panel's own `edge_fdeb_cycles`
/// max (`edge_style_properties.py:286-297`).
///
/// Ponytail: the reference UI's ceiling, not a measurement. Past ~5 cycles the point count
/// is already capped by [`MAX_SEGMENTS`] and each further cycle halves an already tiny
/// step, so a refused 11th cycle loses only refinement. Escape hatch: raise the const.
pub const MAX_CYCLES: u32 = 10;

/// The most iterations the first cycle takes: the reference panel's own
/// `edge_bundle_iterations` max (`edge_style_properties.py:126-133`). Later cycles take two
/// thirds of the one before, so this is also the ceiling on the whole schedule's work.
///
/// Ponytail: the reference UI's ceiling, not a measurement, and the parameter with the
/// weakest claim to one: an iteration is one more pull of every subdivision point toward
/// its partners, so past convergence it converges again and again rather than finding
/// anything. What it buys is that one `u32` cannot name unbounded work. A caller who has
/// measured convergence and wants the repeats pays for them by raising the const.
pub const MAX_ITERATIONS: u32 = 20;
