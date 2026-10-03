# ADR — Graphviz is a docker-only oracle, never a dependency

Status: **accepted** (user, 2026-09-30).

## Context

The user decided the native Graphviz engines (twopi, circo, patchwork, osage, neato, fdp,
sfdp, dot) must match Graphviz's own output. Graphviz is EPL-1.0: it is an oracle and an
algorithm reference, never linked, vendored, or translated line by line into `crates/`.
The motor needs a docker-only Graphviz oracle before p13-gv1 and p13-gv2 can be gated.

## Decision

Graphviz 16.1.0 is pinned by sha256 in `scripts/orch/fetch-refs.sh` and built from the
release tarball into `ge-graphviz-oracle`, a minimal `debian:trixie-slim` image with no
network at run time and no compiler. The image carries the eight layout engines and
`-Tplain` output. The oracle harness `harness/oracle-graphviz.py` runs in the image,
reads the spectral fixtures emitted by `graph-cli emit-spectral-fixtures`, writes DOT,
runs `<engine> -Tplain -Gstart=1`, and records node positions in points keyed by node
id with the graph bounding box.

The fixtures are reused, not re-emitted: `emit-spectral-fixtures` already carries the
bare graph structure (`n`, `source`, `target`) over the exact indexed topology every
layout engine is gated against, with a manifest and sha256. No new Rust emitter is
needed. DOT nodes are named dense `n0..n{n-1}` so the mapping back to the fixture's
`source`/`target` columns is trivial.

## Consequences

- The image builds and all eight engines report `graphviz version 16.1.0`. The last line
  of the build is `dot - graphviz version 16.1.0 (20260904.0139)`.
- Determinism is proven: the oracle run twice over the same fixtures for twopi and circo
  produces byte-identical output (`cmp` is silent for both engines). The `cmp` is not
  vacuous — a negative control that shifts one node coordinate by 1e-6 points makes it
  report a difference at byte 95 and exit 1.
- `-Gstart` is INERT for these two engines, measured rather than assumed: the same
  fixture hashes identically with start=1, 7, 99, and with no `-Gstart` at all. twopi and
  circo are deterministic unconditionally, so the determinism proof rests on the engine,
  not on the seed. A native port therefore cannot be credited with seed stability, and
  p13-gv1 must not gate on one.
- A full 1000-seed circo sweep is not affordable (n=501 alone is ~53 s; the sweep is
  ~1.5 h), so the determinism check runs over a strided 20-seed subset spanning
  n=2..552. The full sweep is p13-gv1's job, not the harness's.
- The plain format reports inches; the harness multiplies by 72 to report points. A
  single-node graph confirms the unit: bbox `0.75 0.5` inches → `54 36` points.
- The differential metric for each engine is the largest absolute coordinate difference
  in points, after both arms are rescaled to the same bounding box. The ceiling is the
  next power of ten above the worst measured gap over 1000 seeds, measured and recorded
  in `docs/measurements/`, never guessed.
- Graphviz source is read as an algorithm reference only. No Graphviz code is linked,
  vendored, or translated line by line into `crates/`.

## Ponytail (Graphviz oracle)

The oracle is a docker-only image with no network at run time. **Failing input:** a host
without docker, or a Graphviz release that changes its layout output between versions —
the pin is a sha256, so a version change is a stop, not a silent drift. **Direction:** a
layout that diverges from Graphviz's output is a red gate, not a wrong number — the
differential measures the gap, it does not hide it. **Escape hatch:** the native engine
is gated against the oracle at a stated ceiling; past the ceiling the engine is
`implemented`, not `gated`, and the ledger says so.

## Node width: one table, measured not derived

`dot` sizes a node from its *rendered* label, so the layered engine's x coordinates depend
on a font metric the motor does not have. The decision is to **pin the metric as a measured
table** in `crates/graph-core/src/layout/graphviz/text_width.rs` — the only copy — rather
than derive it, ship a font engine, or guess. It lives in `layout::graphviz` rather than
under `dot` because `osage` and `patchwork` have the same cause
(`docs/measurements/p13-gv1-osage.md`) and the follow-up is to wire them to this one table.
osage is **not** wired to it in this change; that is named, not done.

Measurement, one-node graphs with the node attributes `width=0 margin=0` so the printed
width is the label alone, read from the plain format's node line:

```text
docker run --rm --pull never --user 0:0 -v "$PWD:/w" -w /w ge-graphviz-oracle \
    dot -Tplain one-node.dot
```

The plain format prints **inches at five significant digits**, so the measurement's own
quantisation is 0.00001 in = 0.00072 pt. Every test in the module compares at that grid,
never finer: a finer comparison would be against digits the oracle never printed. The
`width=0 margin=0` attributes must be **on the node** — putting them in `graph [ ... ]`
leaves the 0.75 in floor in place and the measurement is silently wrong.

The table splits in two, and both halves are separately exact:

| k | inches | points |
|---|---|---|
| 1 | 0.12563 | 9.04536 |
| 2 | 0.26696 | 19.22112 |
| 3 | 0.40830 | 29.39760 |
| 4 | 0.54963 | 39.57336 |
| 5 | 0.69096 | 49.74912 |
| 6 | 0.83230 | 59.92560 |
| 7 | 0.97363 | 70.30136 |
| 8 | 1.11500 | 80.28000 |

With the shape forced to `box` and the margin at zero, the printed width is the label's
own box and it is a whole number of points: **8 points for the first character, exactly 9
for each one after it**, verified for k = 1 to 8. With the default shape the printed width
is the bounding box of the ellipse circumscribing that box, and the ratio between the two
series is constant over k = 1 to 16. So the rule is

```text
text_width_pt = (leading_box + 9 * (k - 1)) * 1.130667
```

The factor is pinned to seven digits because that is the interval the measurements allow:
every inch string above is reproduced by every value in `[1.130661, 1.130670)` and nothing
outside reproduces all eight.

**This corrects the first job's `9.04536 + (k - 1) * 10.1758`.** That advance is
`9 * 1.130667` *truncated*; truncated, it lands 0.40829 in at k = 3, which the plain format
prints as `0.40829`, while the oracle printed `0.4083`. The first job's rows in
`docs/measurements/p13-gv2-dot.md` are each consistent with the measurements to within the
oracle's own 0.00072 pt grid, and mutually inconsistent at four decimal places; the module
keeps the two factors apart instead of storing a base and an advance derived from them, and
a negative-control test asserts the truncated advance does **not** reproduce the table.

**Digits share `n`'s advance** — measured, not assumed: `n0` = `nn` = 0.26696 in, `n10` =
`nnn` = 0.4083 in, `n100` = `n551` = `nnnn` = 0.54963 in. The 1000-seed fixture ids are
`n0`..`n551`, lengths 2, 3 and 4, so the rule covers every label the motor is fed. A
leading `1` is one point narrower than a leading `n` (0.10993 in against 0.12563 in) and
advances by the same 9; a first-glyph side bearing, not a per-character metric.

**Node width is a separate question and it is not settled here.** The formula of record is
`max(0.75 in, text + 2 * 0.11 in)`. The oracle's default node width for the same labels is
0.75, 0.80475, 0.97719 and 1.1496 in for lengths 2, 3, 4 and 5 — the four node-width rows
already in `docs/measurements/p13-gv2-dot.md` — and the formula of record returns 0.75 in for
the first three, so **it does not reproduce them**. The measured relation is
`node = 1.37952 * label_box + 0.30669` inches, the factor coming from the ellipse
circumscribing the label box *plus* the default margin (1.130667 with no margin). Node width
is an input only to the x-coordinate network simplex, which is the position pass and not the
rank pass, so the formula of record is implemented and the discrepancy is left recorded here
for the position pass to settle rather than guessed at in a rank change.

ponytail: the linear rule is verified for k ≤ 8 only. At k = 9 the box series prints 79
points where `8 + 9 * 8` says 80, and the default-shape series runs one point lower from
there on, so a longer id is not covered. **Failing input:** a label over 8 characters, or one
containing a character outside `n` and the ASCII digits, which is given `n`'s leading box
and the common advance with no measurement behind it. **Direction:** the gap is one point
low and widening, so a long non-ASCII id is wrong by an unknown amount, not by a rounding.
**Escape hatch:** the module is the single copy; correcting it is editing one constant pair,
`BOX_ADVANCE_PT` and `ELLIPSE`, and the negative control then has to be updated with it.
