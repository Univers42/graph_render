# Job fix-force-session (agent build: review-layout-force LF-13, LF-18)

Read `prompts/jobs/fix-common.md` first. Source: `docs/reviews/review-layout-force.md` (ids `LF-NN`,
verified on `74f8994`; perf-p4-carry has since added `session/carry.rs`, so re-measure first).

1. **LF-13, MAJOR.** `layout/force/session.rs` is over 300 lines and `step_under` takes 5
   parameters. Split on the existing boundary (the `session/live_params.rs` precedent) and fold
   `workers` + `split` into one value. Behaviour-preserving: paste the session tests and
   `hashgate --seeds 8` before and after.
2. **LF-18, MINOR.** `session/live_params.rs` `validate` has no cross-field check:
   `distance_min > distance_max` and `initial_alpha < alpha_min` validate. RED for each, then GREEN
   inside `ordered()` so the "one order for both" claim stays true.
3. The review's unverified item on `live_params.rs` `validate_finite`: find whether a live setter
   reaches the frozen path (which would break "refuse, never clamp"); a RED test or `false`.

Paths: `crates/graph-core/src/layout/force/session.rs`, `crates/graph-core/src/layout/force/session/**`,
`docs/measurements/fix-force-session.md`.

Done when: fix-common's done-when; both ids have a row; `wc -l` of every file under
`layout/force/session*` is at most 300.
