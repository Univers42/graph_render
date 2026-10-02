//! `_spiral_layout_3d` hand-pinned against
//! `SciGraphs/core/scigraphs_core/mesh/layouts/basic.py:36-63`.
//!
//! **Every constant below is a bit pattern, not a decimal.** The first half is what the
//! port has to reproduce exactly: `t` and `z` involve no transcendental at all, so every
//! digit of them is reachable. The second half (`x`, `y`) is `libm`'s `sin`/`cos` against
//! numpy's, which is a real question with a real answer — see
//! [`reference::the_trig_columns_agree_with_the_reference_to_the_last_bit_the_f32_keeps`].
//!
//! Provenance, so a re-measurement can be checked rather than trusted: run inside the
//! pinned `ge-python-oracle` image (numpy 2.3.3) with the `SciGraphs/` submodule on the
//! path, `_spiral_layout_3d(n, 5.0)` called as `apply_graph_layout` calls it
//! (`dispatcher.py:105`), printed as `float.hex`-equivalent IEEE-754 big-endian words.
//! Numbers transcribed, nothing retyped.
//!
//! The suite is in three children: [`reference`] holds the oracle's words and everything
//! compared against them, [`structure`] the properties of the construction, and
//! [`degenerate`] the `n = 0` divergence and the drawing's extent.

#[cfg(test)]
mod degenerate;
#[cfg(test)]
mod reference;
#[cfg(test)]
mod structure;
