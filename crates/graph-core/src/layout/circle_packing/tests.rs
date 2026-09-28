//! Circle packing's own test suite. The whole module was a one-line stub before this
//! phase, so every test below was RED — `unresolved import` / `cannot find function
//! 'run' in module 'circle_packing'` / `cannot find type 'CirclePackingParams'` — the
//! moment it was written, before a line of the module's real code existed.

mod determinism;
mod exact;
mod fallback;
mod small;
mod support;
