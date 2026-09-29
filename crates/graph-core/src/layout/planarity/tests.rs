//! Integration tests for [`super::planar_embedding`], [`super::faces`],
//! [`super::euler_certificate`] and [`super::triangulate_embedding`]: known non-planar
//! graphs (K5, K3,3, and subdivisions of both) must fail; known planar graphs (trees,
//! cycles, grids, wheels, outerplanar fans, seeded maximal planar graphs) must succeed
//! with a structurally valid, certified embedding.
//!
//! The whole `planarity` module was a one-line stub before this phase, so every test
//! below was RED — `unresolved import` / `cannot find function 'planar_embedding' in
//! module 'planarity'` — the moment it was written, before a line of the module's real
//! code existed; the phase report records the observed compiler output from that first
//! build.

mod checks;
mod faces;
mod graphs;
mod negative;
mod positive;
mod properties;
mod sweep;
mod triangulate;
