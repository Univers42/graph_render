//! The POST stage (`prompt.md` §3): everything that runs **after** a layout and turns
//! node positions into edge geometry. In SciGraphs this separation is deliberate —
//! `mesh/edge_styles.py` and `engine/bundling/*` sit downstream of every layout, which
//! is why a POST capability composes with all of them rather than being re-implemented
//! per layout.

pub mod styles;
