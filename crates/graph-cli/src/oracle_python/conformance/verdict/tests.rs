//! The judge, held to inputs this module writes itself.
//!
//! **These are the tests that make the gate non-vacuous.** Each of the checks in
//! [`super::one_row`] gets one test that fails it, and the one that must pass. The fixture
//! directory they run against holds **real bytes**, because the judge re-hashes the `.f64`
//! files rather than trusting the digest an arm recorded: a test with no bytes behind the pin
//! would be measuring exactly the trust this module exists to replace.
//!
//! **One file per behaviour under test.** `harness.rs` builds the fixture directory and the
//! pins; `rows.rs` holds what happens to a row whose bytes or shape moved; `run.rs` holds
//! what `run` refuses before it judges a row at all; `pins.rs` holds the two rules that let
//! `GRAPHVIZ_DOT` pass unmeasured. A test lands in the file named for the behaviour it fails.

mod harness;
mod pins;
mod rows;
mod run;
