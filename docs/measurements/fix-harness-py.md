# fix-harness-py — the Python oracle arms

Repair of `docs/reviews/review-harness-sdk.md` ids **B3, B4, M15–M23, M26–M29, M37**,
**m44–m60, m62–m71, m77, m106–m112**, plus the unverified items **U5, U7**.

Rules of record: `prompts/jobs/fix-common.md`, `CLAUDE.md`, `prompt.md` §6 (D1–D10).

**Shared module added**: `harness/oracle_common.py`. Nine arms already carried the same
five digest lines and three more copied `manifest["sha256"]` into their result instead of
the digest of the bytes they read (B3), and the empty-set / non-finite refusals were
written out separately in nine places. It exports `read_manifest`, `read_json`,
`require_seeds`, `require_cases`, `finite`, `networkx_version`. Every arm this job touched
calls it.

**No arm was made looser.** No ceiling was raised, no case dropped, and every gated number
is byte-identical to the pre-fix arm on the committed fixtures — recorded per row below.

## Verdicts

| id | severity | verdict | test / evidence | file:line |
|---|---|---|---|---|
| B3 | BLOCKER | fixed | truncate `twopi.jsonl` to 1 seed: RED exit 0 recording digest `9d439abd…` for bytes hashing `fe195582…`; GREEN exit 1 `twopi.jsonl does not match its manifest` | `oracle_common.py:47`, `oracle-twopi.py:265`, `oracle-graphviz.py:288,335` |
| B4 | BLOCKER | fixed | `emit-closed-form-fixtures --seeds 0`: RED exit 0 `{"ring":{"cases":0,"worst":0.0}}`; GREEN exit 1 `closed-form.jsonl compared no ring case` | `oracle_common.py:74`, `oracle-closed-form.py:118` |
| M15 | MAJOR | fixed | `--start=7` on `fdp`: before, the framed cases ran at seed 1 (`closed_exact: true`); after, at 7 (`closed_exact: false`) — the seed is now load-bearing as `p13-gv2-fdp.md:364` claims. No gate row runs a differential at a non-default `--start` | `oracle-graphviz.py:213` |
| M16 | MAJOR | fixed | `--start=7` record arm → manifest now `"start": 7` (was the `START_SEED` literal) | `oracle-graphviz.py:268` |
| M17 | MAJOR | fixed | result's `oracle` is interpolated: `Graphviz 16.1.0 twopi -Tplain -Gstart=7` at `--start=7` | `oracle-twopi.py:279-283`, `gv_plain.py:75-89` |
| M18 | MAJOR | fixed | `graphviz_version()` parses `dot -V`; `test_another_version_is_refused` RED on pre-fix `gv_plain`, GREEN after | `gv_plain.py:75-89`, `test_gv_plain.py` |
| M19 | MAJOR | fixed | 1-line fixture + re-sealed digest: RED `cases: 1` exit 0; GREEN exit 1 `osage.jsonl holds 1 cases, want the manifest's 20 seeds` | `oracle_common.py:62`, `oracle-graphviz.py:290` |
| M20 | MAJOR | **deferred** | measured: negating `spectral.x/y` scores 0.0 (invariant). The proposed signed gate moves the honest row red — the port and the reference differ by a **rotation**, not a sign, inside a non-degenerate span. `oriented_worst` now records it | `oracle-spectral.py:96,190` |
| M21 | MAJOR | **false** | measured: `layout_kamada_kawai(dim=3)` **refuses** (`Invalid start position matrix size in 3d Kamada-Kawai layout`); `drl(dim=3)` moves worst 0.494→0.894 by scoring a 3-D drawing against a 2-D stress ratio. `dim` is 2 for this metric and is now explicit + shape-checked | `oracle-igraph.py:120-136` |
| M22 | MAJOR | fixed | NaN in `spectral.x[0]`, re-sealed: RED `worst 0.0` exit 0; GREEN exit 1 `non-finite … gap` | `oracle_common.py:83` + every accumulator in 9 arms |
| M23 | MAJOR | fixed | empty + re-sealed `spectral.jsonl`: exit 1 `spectral.jsonl compared no spectral case` | `oracle-spectral.py:214` |
| M26 | MAJOR | fixed | stub `sfdp` exiting 1 with empty stderr: RED returned the drawing; GREEN exit 1. Test `test_a_failing_engine_that_wrote_nothing_to_stderr_is_refused` | `gv_plain.py:104-112`, `test_gv_plain.py` |
| M27 | MAJOR | fixed | the file's own documented invocation `fa2-chaos.py <dir> 5,10,20,40,100`: RED exit 0 with every row "not compared"; GREEN exit 1 `no requested budget equals the gated max_iter=2`. The default list now leads with the gated budget | `fa2-chaos.py:86,283`, `fa2.rs:44-53` |
| M28 | MAJOR | fixed | the script was already correct; `m70`'s bytes-before/after assertion is now in it, and a no-op perturbation refuses | `perturb-closed-form.py:46-52` |
| M29 | MAJOR | fixed | emitter writes via `mkstemp`+`fsync`+`os.replace` behind a `--out` flag (no shell redirect to truncate the committed file); provenance gains `source_digests` (16 entries) and `scigraphs_revision`. Crash controls leave the fixture intact | `emit-scigraphs-lesmis.py:44`, `scigraphs_lesmis_graph.py:86,119`, `scigraphs_lesmis_fixture.py:171` |
| M37 | MAJOR | fixed | the arm now reads the committed file and asserts it equals `emit_document(...)`. Tampering `nodes[0].screen` in a scratch copy: RED all-12-pass, GREEN exactly the new test fails | `test_scigraphs_lesmis_document.py:52-63` |
| m44 | MINOR | fixed | empty `fa2.jsonl` + re-sealed: exit 1 `fa2.jsonl compared no fa2 case` | `oracle-fa2.py:96` |
| m45 | MINOR | fixed | `worst_seed` is now `case["seed"]`, not the list ordinal | `oracle-fa2.py:99` |
| m46 | MINOR | fixed | `extent <= 0` refused (`the reference drawing has extent 0.0`) instead of dividing by `1e-300`; every gap through `finite` | `oracle-fa2.py:73-75` |
| m47 | MINOR | fixed | CWD-independent `sys.path`; negative control runs from `/w/crates/graph-cli` → exit 1 `ModuleNotFoundError: scigraphs_core` | `oracle-spring.py:44-51` |
| m48 | MINOR | fixed | `threshold=` dropped from `nx.spring_layout` (SciGraphs' own SPRING does not pass it); `cmp` of theirs with/without is byte-identical at the 1e-4 default, so the row does not move | `oracle-spring.py:84-89` |
| m49 | MINOR | fixed | explicit `n == 1` branch against the reference's **origin** (`basic.py:86-89`), which `min(n,8)` had been scoring against `CORNERS[0]*scale`. The pair inverts under the old code | `oracle-basic-3d.py:108,126-131` |
| m50 | MINOR | fixed (doc) | both interior stats are still emitted; the file and docstring now say they are **recorded and not gated** and name the Rust field that would read them. `basic_3d.rs` is not in this job's paths | `oracle-basic-3d.py:34-43,226-229` |
| m51 | MINOR | fixed | `pivot_spectrum` calls `eigh` **once** and captures from that call; it used to record eigenvalues from a *different* decomposition than the vectors returned | `oracle-spectral.py:139-165` |
| m52 | MINOR | **deferred** | measured: removing the tie relaxation moves `pivot_mds` 1.977e-08 → 1.946 (red). The review's premise — that the reference's sign is deterministic *and we match it* — does not hold for our port. The relaxation stays, documented with that measurement | `oracle-spectral.py:78-101` |
| m53 | MINOR | fixed | the reference's progress/fallback prints go to stderr; stdout is one JSON document | `oracle-spectral.py:207-209` |
| m54 | MINOR | fixed | a collapsed drawing (`denom <= 0`) is skipped, not scored with a fabricated `scale = 0.0` | `oracle-igraph.py:150-152` |
| m55 | MINOR | fixed | `dim` explicit where igraph accepts it, `None`+shape-check where it refuses it (measured), and the returned width is checked against `DIMENSION` | `oracle-igraph.py:120-136,166-179` |
| m56 | MINOR | fixed | one open, one pass: the digest and the measurement read the same handle's bytes | `oracle-closed-form.py:44-47` |
| m57 | MINOR | fixed | `require_seeds`; the closed-form `--seeds 1` truncation now refuses | `oracle-closed-form.py:47` |
| m58 | MINOR | fixed | both networkx layouts hoisted out of the per-node comprehension (were rebuilt `n` times per case) | `oracle-closed-form.py:77-86` |
| m59 | MINOR | fixed | `networkx_version()` refuses a networkx that is not the pinned 3.6 | `oracle_common.py:97` |
| m60 | MINOR | **false** | `oracle_python.rs:189-190` already refuses when `manifest["fingerprint"] != result["fingerprint"]` **or** the manifest's is not this tree's; the arm's own digest check binds the fixture bytes. Comment added naming the checker | `oracle-circular-hierarchy.py:104-105` |
| m62 | MINOR | fixed | `framed_cases` returns `(compared, skipped)`; `sfdp` records `closed_skipped: [five-star, four-cycle, six-branch, three-path, two-nodes]` beside its 1-of-6 verdict. An engine that compared nothing is refused | `gv_frames.py:113-142`, `oracle-graphviz.py:302-304` |
| m63 | MINOR | fixed | `framed_case` and `gv_closed.closed_case` down to 4 parameters; `start=START_SEED` no longer bound at `def` time (test rebinds the module attr and reads the seed back off the command). `engine_points` keeps 6 positional parameters **on purpose** — `sc_graphviz.py:59` calls it and another job owns that file | `gv_frames.py:103`, `gv_closed.py:168`, `gv_plain.py:158` |
| m64 | MINOR | fixed | `printed_nodes` refuses a missing graph line and a wrong node count; RED was `KeyError: 'n1'` / `KeyError: 'n0'` | `gv_plain.py:161-198` |
| m65 | MINOR | fixed | `subprocess.run(..., timeout=ENGINE_TIMEOUT)`, a `TimeoutExpired` is a refusal; carries a `Ponytail:` line on what the flat ceiling gets wrong | `gv_plain.py:52-61,99-102` |
| m66 | MINOR | fixed | per-case guard; a malformed line is recorded against its seed and the sweep continues, exiting 1 with the failed seeds named | `fa2-chaos.py:198-221,257-274` |
| m67 | MINOR | fixed | each case's own `params`; disagreement between cases is refused | `fa2-chaos.py:209,249-263` |
| m68 | MINOR | fixed | provenance carries `max_iter`, `jitter_tolerance`, `scaling_ratio`, `gravity` | `fa2-chaos.py:80,266` |
| m69 | MINOR | fixed | `ours_vs_nx` accumulates only at the gated budget; off-budget worst seeds stay `None` | `fa2-chaos.py:176-182` |
| m70 | MINOR | fixed | bytes-before/after compared; a no-op perturbation exits 1. Control run on all three with the mutation commented out | `perturb-*.py:46-52` |
| m71 | MINOR | fixed | `mkstemp` + `os.replace` for both the jsonl and the manifest, with the scratch-dir invariant stated | `perturb-*.py:23-40` |
| m77 | MINOR | fixed | usage string + arity check on `oracle-fa2.py` and `oracle-spring.py` (both were a bare `IndexError`) | `oracle-fa2.py:88-89`, `oracle-spring.py:57-58` |
| m106 | MINOR | fixed | `scigraphs_lesmis_graph.py` reads the same `SCIGRAPHS_ROOT` env var as `scigraphs_lesmis_pins.py`; control shows both agree, unset → `/sg` | `scigraphs_lesmis_graph.py:36` |
| m107 | MINOR | fixed | `colormaps.py:534-536` (the QUANTILE branch) → `:527-530` (RANK), verified by reading the file | `scigraphs_lesmis_fixture.py:44` |
| m108 | MINOR | fixed | 3 of 9 provenance paths did not resolve (`geometry.py`, `executor.py` ×4, `text_overlay.py` ×3 — all under `SciGraphs/SciGraphs/core/…`); all 16 entries resolve now | `scigraphs_lesmis_fixture.py:36,38,39,48,51` |
| m109 | MINOR | deferred | `world_bounds([])` still returns `((), RADIUS_FLOOR, (), ())`. `scigraphs_lesmis_camera.py` has **no finding of its own** and its `WorldBounds` tests are in a file owned by another job; refused here under "decisions needed" | — |
| m110 | MINOR | fixed | `check_label_ids()` refuses any id outside `[0, NODE_COUNT)`, called at import. Controls: `-1`, `77`, `-2`, `200` all refused naming the id and the bound | `scigraphs_lesmis_pins.py:60,86` |
| m111 | MINOR | fixed | the gallery counts are built from `pins.NODE_COUNT`/`IN_FRAME`/`UNOCCLUDED`/`KEPT` instead of literals; the one count with no independent origin is named as a transcription in the docstring | `test_scigraphs_lesmis_document.py:43,170-192` |
| m112 | MINOR | deferred | no row runs any `test_scigraphs_lesmis*` module. `scripts/orch/rows/*.rows` is not in this job's paths; the exact row text is under "decisions needed" | — |
| U5 | unverified | closed | all four arms' evidence needed `ge-python-oracle`/`ge-graphviz-oracle`; both images are present and every arm above was **run**, not read | — |
| U7 | unverified | closed | `oracle-fa2.py`'s empty-set (m44), `gv_frames`' silent drop (m62) and `fa2-chaos`' "not compared" (M27) claims were each **run**, with the reproducer pasted in this table | — |

## Commands and last lines

Merge floor:

```
scripts/orch/gr cargo fmt --all --check                                     -> 0
scripts/orch/gr cargo clippy --workspace --all-targets -- -D warnings      -> 0
scripts/orch/gr cargo build -p graph-core --target wasm32-unknown-unknown   -> 0
scripts/scigraphs-conformance.sh                                           -> 0  PASS
scripts/scigraphs-conformance.sh --break                                   -> 1  scigraphs-conformance: --break caught: SPRING_3D
scripts/orch/gr cargo test --workspace --no-fail-fast                       -> 101  (one pre-existing failure, see below)
```

`cargo test` — 1235 pass, 1 fail:
`layout::graphviz::dot::rank_tests::the_first_twenty_fixture_seeds_rank_as_the_oracle_ranks_them`
panics at `crates/graph-core/src/layout/graphviz/dot/oracle_probe.rs:58` on a **missing
probe file** `target/probe/rank1000.txt` (untracked, absent from a clean checkout).
Pre-existing and outside this job's paths: nothing under `crates/graph-core` was touched,
and the module's own doc says the tests that read the probe should skip rather than fail.

Every arm's verdict on its committed fixtures (`graph-cli oracle-*`):

```
oracle-twopi PASS | oracle-graphviz --engine osage PASS | oracle-spring PASS
oracle-closed-form PASS | oracle-fa2 PASS | oracle-spectral PASS | oracle-igraph PASS
oracle-basic-3d PASS | oracle-circular-hierarchy PASS | oracle-hierarchical-3d PASS
```

Conformance, 32 rows, `pass: true`, every row `unexplained: 0` and every
`procrustes_median` under its pinned ceiling (`target/gates/scigraphs-conformance.json`).

New tests: `harness/test_oracle_common.py` (12) and `harness/test_gv_plain.py` (14), both
green. Run against the **pre-fix** `gv_plain`, 7 of the 14 fail — including M26's exact
reproducer and `KeyError: 'n0'`/`'n1'` for m64 — so they are not vacuous.

```
docker run … ge-python-oracle  python3 -m unittest discover -s harness -p 'test_oracle_common.py'  -> 0  Ran 12, OK
docker run … ge-graphviz-oracle python3 -m unittest discover -s harness -p 'test_gv_plain.py'       -> 0  Ran 14, OK
```

## Deferred, with the decision each waits on

- **M20 / m52** — a signed or rotation-aware gate on `spectral` / `pivot_mds` turns a green
  row red. Measured, on identical bytes: `spectral` 0.0 → 1.527, `pivot_mds`
  1.977e-08 → 1.946. Diagnosis: both arms pin signs by the same rule
  (`networkx_layouts._fix_eigenvector_signs` and `linalg::pin_signs`), yet the columns still
  disagree — by a **rotation** on some components (seed 3: same 1.527, flipped 0.678,
  swapped 0.527), i.e. the port's eigenbasis inside a non-degenerate 2-D span is not the
  reference's. That is a `graph-core` property, not a harness defect.
  **Recommended:** fix the port's basis, then gate on `oriented_worst`. Until then the
  number is recorded in every result file, so the divergence is visible rather than silent.
- **M28's row** — the perturbation script is correct and now asserts it changed bytes, but
  the row that would run it (`develop-full.rows`, after `:193`) is not in this job's paths.
  **Recommended:** `oracle-closed-form-perturb-negctl|nonzero|… perturb-closed-form.py …`.
- **m112's row** — **Recommended:**
  `lesmis-tests|0|docker run --rm --pull never --user 0:0 -v "$PWD:/w" -v "$PWD/SciGraphs:/sg:ro" -w /w ge-python-oracle python3 -m unittest discover -s harness -p 'test_scigraphs_lesmis*.py'`
  and one for the emitter (M29's "no gate row runs the emitter" is the same gap).
- **m109** — refused `world_bounds([])`. `scigraphs_lesmis_camera.py` is in this job's
  paths but `test_scigraphs_lesmis_branches.py`'s `WorldBounds` cases are not, and the file
  has no finding of its own. **Recommended:** add the refusal with a matching test case.
- **m107 / m108 fixture staleness** — correcting the eight provenance strings made the
  committed `fixtures/scigraphs/lesmis.json` differ from the emitter, which M37's new test
  correctly reports as the one failing case among 64. `fixtures/` is not in this job's paths.
  **Recommended:** the fixture owner re-emits with
  `emit-scigraphs-lesmis.py --out fixtures/scigraphs/lesmis.json`, which is now atomic and
  so cannot leave a truncated fixture. Everything outside `provenance` is byte-identical:
  nodes, edges, labels, radii and params all compare equal.

## Also found, not in the review

- `harness/__pycache__/` had appeared **inside the fingerprinted set** (`fingerprint.rs:27`)
  — a root-owned `.pyc` from an arm run that did not set `sys.dont_write_bytecode`. Removed,
  and every module this job added or edited sets the flag first. Two test files that import
  the modules directly were the source; they set it too now.
- `docs/contract/wasm-abi.md` and the review's M21 both treat `dim=3` as the reference's
  setting for the igraph family. For **this arm** the metric is a two-dimensional stress
  ratio, and igraph refuses a 3-D Kamada-Kawai start matrix outright. M21 is `false` on
  measurement, not on judgement.
- `oracle-basic-3d.py`'s docstring claims the interior is "strictly inside the shell".
  Nothing measures the shell bound; only the mean and the variance are computed. Left as-is
  (not a finding id) and reported.