# Job sg-grid-minors (agent build; five review minors on sg-grid-scale, comments, test names and docs)

Why: the review of sg-grid-scale (graph-render-3e, 2026-10-02) approved landing with five minors. No
behaviour change is wanted except (2), where you may pick the refusal.

Do:
1. `scaled.rs:161-172` (the grid layout's `scaled.rs`; `git grep -n a_f32_pitch_would_not_have_reached_these_bits`):
   the test never runs the kernel, so it is not a negative control. Make it call `run_scaled`, or rename it
   for what it checks. The real guard is `every_node_matches_the_reference_formula` at n=601.
2. `scaled.rs:58-60`: "refuses instead of writing a non-finite coordinate" is false for a finite scale
   above ~6.8e38, which overflows f32 to inf. Either fix the comment, or refuse when
   `(cols-1)*scale/cols` overflows f32 (with a test at 1e39). Say which and why.
3. `crates/graph-cli/src/oracle_python/conformance/gaps.rs:30`: the `G_GRID_ITER` anchor should name the
   `pub fn run_with(` line of `grid.rs` (100, not 101). Check the line on the tree you have.
4. `docs/measurements/scigraphs-conformance.md` lines ~90-95 and ~168-176 are stale for GRID: it is now
   f32 1020/1020, so 5 rows are f32-identical, not 4; update the old 4.0-gap text.
5. `docs/measurements/sg-grid-scale.md`: add the fmt/clippy/test last lines (from your own runs).

Done when: quick.rows green (hashgate-8 and its negctl included), `scripts/scigraphs-conformance.sh`
exit 0 and its `--break` exit 1, with every exit code pasted.
