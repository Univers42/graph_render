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
