//! The one node cap the bench campaign will build, owned by the motor.
//!
//! `graph-cli bench` parses `--n` against a range whose top is this figure
//! (`crates/graph-cli/src/bench.rs:63`, `crates/graph-cli/src/bench/tick.rs:47`), so no
//! measurement of any layout can be taken at a larger size. Several registry ceilings
//! answer with exactly this number; the point of one constant is that they *are* it
//! rather than two literals that agree today.
//!
//! **It lives here, not in `graph-cli`, because the dependency runs one way.**
//! `graph-cli` depends on `graph-core` (`crates/graph-cli/Cargo.toml:13`) and is a
//! bin-only crate with no `[lib]` target, so `graph-core` cannot name
//! `bench::scale::MAX_SCALE_NODES` at all. `bench/scale.rs` therefore derives its own
//! `MAX_SCALE_NODES` from this constant rather than the reverse, and the registry's
//! ceilings are checked against it by
//! `registry::tests::the_radial_and_basic_3d_ceilings_are_the_one_bench_node_cap`.
//!
//! **It is a cap, not a wall.** Nothing in this crate stops working at this node count:
//! the graph-free placements are O(n) in three `f64` columns, and `layout.twopi` is
//! O(n + m). Every row that quotes this figure carries a `Ponytail (scale_ceiling)`
//! line saying the same. This constant itself is exact — it is the top of a parse
//! range, not a bound estimated from a run — so it owes no Ponytail of its own.

/// The largest node count `graph-cli bench` will build: ten components of
/// `synthetic::MAX_SYNTHETIC_NODES` (`crates/graph-core/src/synthetic.rs:33`), which is
/// the same 100 000 `bench::scale::COMPONENT_NODES` names.
pub const MAX_BENCH_NODES: u32 = crate::synthetic::MAX_SYNTHETIC_NODES * 10;
