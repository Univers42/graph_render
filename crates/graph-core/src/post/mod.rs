//! The POST stage (`prompt.md` §3): everything that runs **after** a layout and turns
//! node positions into edge geometry. In SciGraphs this separation is deliberate —
//! `mesh/edge_styles.py` and `engine/bundling/*` sit downstream of every layout, which
//! is why a POST capability composes with all of them rather than being re-implemented
//! per layout.
//!
//! This module holds the spatial substrate: [`grid_index`], a uniform grid over the node
//! geometry with node cells marked as obstacles, built into a reused buffer; and
//! [`routed`], obstacle-avoiding routing over that grid.

pub mod grid_index;
pub mod routed;
