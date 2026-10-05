//! The shared test support: the database handle and the file handshake.
//!
//! WHY the dead-code lint is off for this module and only this module: every test file declares
//! `mod support;`, so each test binary compiles the WHOLE support module and then uses only the
//! parts its own cases need. Without this, `cargo clippy --all-targets` fails on the helpers a
//! given binary happens not to call — which is the shape of the module, not a defect in it.

#![allow(dead_code)]

pub mod case;
pub mod db;
pub mod fixture;
pub mod rng;
pub mod step;
pub mod workspace;
