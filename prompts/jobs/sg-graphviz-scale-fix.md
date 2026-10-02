# Job sg-graphviz-scale, review round 1 (same session, same worktree): FIX, 2 MAJOR

The review (graph-render-3e, 2026-10-02) says your "partial" is honest, but two claims under it are
wrong. Fix them, then take the path past partial.

MAJOR 1 — `-Tplain` is `%.5g` (5 significant digits), not five decimals: graphviz-16.1.0
`lib/common/output.c:66-71` (`printdouble "%.5g"`) and `:76-79` (`printpoint`, in inches). The step is
1e-4 in (7.2e-3 pt) for 1-10 in and 1e-3 in (0.072 pt) above 10 in: at least 10x your 7.2e-4 pt grid,
so the step sizes in the report (6.9e-6 lesmis, 1.7e-5 gate-03) are off by at least 10x. TWOPI's max_gap
7.46e-5 fits `%.5g` (half-step 8.3e-5 on gate-03), not your claim. Fix every mention and recompute the
steps: `gaps.rs:100`, `harness/scigraphs-conformance/sc_graphviz.py:13-14,129`,
`docs/measurements/sg-graphviz-scale.md:45-52,93,129`, `docs/measurements/scigraphs-conformance.md:62-63,174,203,280-281`.

MAJOR 2 — numpy's pairwise summation applies only along the fast memory axis. SciGraphs does
`raw.mean(axis=0)` on an (n,2) array (`SciGraphs/.../yifan_hu.py:309,318`). C-contiguous means each
column is summed left to right, so `left_to_right` (your negative control) is probably the correct order.
Your probe (`gv_post/tests.rs:93-100`) and the numpy probes were 1-D; the only 2-D check was n=3, where
both orders agree. Pin against numpy `raw.mean(axis=0)` on an (n,2) C-contiguous array at n >= 9 in
`ge-python-oracle` (or prove the array is not C-contiguous), switch the order if so, and add a `Ponytail:`
line naming the order and its evidence. Places: `gv_post.rs:43-50,88-106`, `sc_graphviz.py:117,124`,
`gv_post/tests.rs:70-90`.

MINOR:
- "a layout call, not a rendering, so no rounding" (scigraphs_utils) is inference: no source is on
  disk, only the `constraints/linux-x64.txt:21` pin. Mark it unverified.
- `docker/graphviz-oracle.Dockerfile:9` says "no compiler", but `:24` installs build-essential: make the
  comment true.
- Your log ran `clippy -p graph-cli`, but the report says `--workspace`: run `--workspace`.

Path past partial (do this after the fixes): every Graphviz text output rounds (plain/plain-ext `%.5g`
in; dot/json pos `%.5g` pt, `output.c:294,302`; xdot `%.02f`; json draw `%.03f`; pygraphviz goes
through -Tdot). The cheapest exact reference is a ~30-line C reader built inside `ge-graphviz-oracle`
with its gcc against `/opt/graphviz/include/graphviz/{gvc.h,types.h}`: `gvLayout`, then print
`ND_coord(n)` with `%a` or `%.17g` (the same translated points `-Tplain` rounds). Wire it in as the
reference, then re-measure TWOPI and PATCHWORK: bitwise, or a measured tolerance with its reason.

Done when: quick.rows green (clippy `--workspace`, hashgate-8 and its negctl), `scripts/scigraphs-conformance.sh`
exit 0 and `--break` exit 1, the numpy (n,2) pin pasted, the TWOPI/PATCHWORK re-measure recorded in
`docs/measurements/sg-graphviz-scale.md`, and the return block with every real exit code.
