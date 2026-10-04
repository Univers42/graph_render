# Perf P6 — a reveal step is 99 % `styleInputOf`'s caller: the memos move 1 ms of a 117 ms restyle, and the digest is 0.66 % of the load

Measured 2026-10-03 in worktree `perf-p6-data-path` (all changes uncommitted; the orchestrator
commits), image `gm-chromium`, `GM_GPU=1`, 1 000 000 nodes, snapshot `60777740` bytes,
`nodes: 1000000`. Renderer, printed by every probe:

```text
ANGLE (AMD, Vulkan 1.4.305 (AMD Radeon RX 6600 (RADV NAVI23) (0x000073FF)), radv)
```

Two round sets of three (`pre`, `post`), arms alternated within each set; every headline number
below is a median **including** its worst round, and the per-round values name it.

**Caveat:** host load average was not retained per round. The machine was shared with other jobs
throughout, which is what most of the spread below is — `revealStepMs` alone runs 210–311 ms
across `pre`'s three rounds. The two sets are not a controlled pair, so read the small
differences as noise and the attribution in §4 as the explanation.

## Reproduction

The reveal step has no stock probe: `deploy/perf/probes/` covers `open`, `settle`, `zoom`,
`orbit3d`, `block`, `frame`, `idle` and `stats`, none of which drives a reveal or reads a digest.
Both round sets therefore came from a throwaway probe under `/tmp/opencode/p6-datapath/`, outside
the repo because only `docs/`, `src/look/`, `pipeline.ts` and `tests/` were editable in this job.
**Caveat:** a host restart removed `/tmp` after the runs, so `probe.py`, `rounds.sh`, `medians.sh`
and the raw per-round logs are gone; the tables below are the surviving record of what they
printed. The method is stated in full so the probe can be rebuilt:

* it served the built `app/dist` and drove `window.__perf.open` / `window.__perf.run`
  (`deploy/perf/drivers/hook.js`), the studio's public actions;
* a **reveal step** is `median(gap between consecutive `view.setStyle` calls) − median(bare-timer
  wait)`. `view.setStyle` runs *after* `styleInputOf` and `styleFrom`
  (`packages/graph-studio/src/studio/pipeline.ts:124`), so timing the call alone measures only the
  call — a `setStyle`-only wrapper reads 0.76 ms and misses essentially the whole step;
* the **bare-timer wait** is the same await with no style work, subtracted because the driver
  waits between steps;
* the **digest** was read by wrapping the worker's digest entry with `self.__gmDigests`
  (`window` is undefined in a worker), once at load and once on an equal-size buffer with the copy
  the digest forces included;
* `colourChange` and `filterChange` are the same gap measurement around a settings change that
  moves `colourBy` and `filter` respectively.

Commands that were run and can be re-run:

```sh
scripts/studio.sh build                                   # build first; the probes never build
scripts/studio.sh check                                   # rc 0
scripts/studio-filters.sh; STUDIO_FILTERS_BREAK=1 scripts/studio-filters.sh
scripts/studio-smoke.sh;    STUDIO_SMOKE_BREAK=1 scripts/studio-smoke.sh
scripts/orch/node-slim.sh node --experimental-strip-types --test \
  packages/graph-studio/tests/reveal-data-path.test.ts    # 7 pass
scripts/orch/node-slim.sh node --experimental-strip-types --test \
  packages/graph-studio/tests/source-identity.test.ts     # 4 pass
scripts/orch/node-slim.sh node --experimental-strip-types --input-type=module \
  -e "$(cat /tmp/opencode/p6-datapath/attr.mjs)"          # §4's attribution
```

## 1. Before — develop, three rounds

| key | round 1 | round 2 | round 3 | median |
|---|---:|---:|---:|---:|
| `loadOpenS` | 10.28 | 10.20 | 10.96 | **10.28** |
| `loadFirstFrameMs` | 146 | 145 | 146 | **146** |
| `digestAtLoadMs` | 47.455 | 43.35 | 44.575 | **44.575** |
| `digestEqualSizeMs` (copy + digest) | 67.65 | 84.8 | 71.02 | **71.02** |
| `digestSharePct` | 0.658 | 0.831 | 0.648 | **0.658** |
| `timerWaitMs` | 40.14 | 40.172 | 40.16 | **40.16** |
| `revealSteps` | 16 | 18 | 13 | **16** |
| `revealStepMs` | 245.025 | 210.398 | 310.715 | **245.025** |
| `revealSetStyleMs` | 0.76 | 0.738 | 0.825 | **0.76** |
| `colourChangeMs` | 237.315 | 259.86 | 299.44 | **259.86** |
| `colourChangeSetStyleMs` | 0.74 | 0.77 | 0.775 | **0.77** |
| `filterChangeMs` | 191.79 | 185.33 | 330.79 | **191.79** |
| `filterChangeSetStyleMs` | 0.755 | 0.74 | 1.185 | **0.755** |

## 2. After — the branch, three rounds

| key | round 1 | round 2 | round 3 | median |
|---|---:|---:|---:|---:|
| `loadOpenS` | 10.45 | 11.25 | 9.86 | **10.45** |
| `loadFirstFrameMs` | 145 | 145 | 145 | **145** |
| `digestAtLoadMs` | 41.55 | 41.64 | 41.805 | **41.64** |
| `digestEqualSizeMs` (copy + digest) | 67.85 | 68.71 | 66.655 | **67.85** |
| `digestSharePct` | 0.649 | 0.611 | 0.676 | **0.649** |
| `timerWaitMs` | 40.107 | 40.1 | 40.112 | **40.107** |
| `revealSteps` | 16 | 16 | 17 | **16** |
| `revealStepMs` | 247.303 | 244.015 | 234.461 | **244.015** |
| `revealSetStyleMs` | 0.75 | 0.755 | 0.75 | **0.75** |
| `colourChangeMs` | 340.25 | 311.13 | 324.34 | **324.34** |
| `colourChangeSetStyleMs` | 1.2 | 1.24 | 1.155 | **1.2** |
| `filterChangeMs` | 219.97 | 228.915 | 187.305 | **219.97** |
| `filterChangeSetStyleMs` | 0.735 | 0.8 | 0.83 | **0.8** |

What moved: the load is the same (10.28 → 10.45 s), the first frame is the same (146 → 145 ms), and
the reveal step is the same (245.025 → 244.015 ms, inside its own pre-spread of 210–311).
`colourChangeMs` reads *higher* after (259.86 → 324.34 ms). Nothing in the change can add that:
the memos allocate a four-element key array per call and rebuild exactly what a `colourBy` change
invalidates, which is what develop rebuilt anyway. §4's attribution puts a `colourBy` miss at
7.008 ms, so the ~65 ms is host noise on a shared machine, not a regression this code can produce.

## 3. The digest's share of the load — step 5, and the stop

`digestSharePct = digestEqualSizeMs ÷ loadOpenS × 100`, the equal-size arm because that is the one
that includes the copy the digest forces. Median **0.658 %** before and **0.649 %** after — checked
against all six per-round values (67.65/10280 = 0.658, 84.8/10200 = 0.831, 71.02/10960 = 0.648,
67.85/10450 = 0.649, 68.71/11250 = 0.611, 66.655/9860 = 0.676).

0.658 % is two orders of magnitude under the 10 % threshold, so the job's step 5 ends here: the
digest is **reported and left alone**. No change was made to it, and none is proposed — changing it
would alter the action-result contract the worker publishes for a number worth 67.85 ms of a
10.45 s load.

**Caveat:** the share is a ratio of two medians from the same round, not a median of the per-round
ratios; the per-round ratios are shown above and the two agree to three digits.

## 4. Where a restyle's time actually goes

The browser numbers alone do not say whether the memos had anything to move, so the same 1M graph
was timed in Node through `scripts/orch/node-slim.sh`, five rounds, median of each:

| what | median ms | share of the 116.685 ms it can see |
|---|---:|---:|
| `styleInputOf`, first call on a new graph — what **every** restyle cost before | **1.566** | 1.3 % |
| `styleInputOf`, a reveal step after it, memos hit | **0.689** | 0.6 % |
| `styleFrom`, on that input — runs on **every** restyle, before and after | **115.207** | 98.7 % |
| both together, one restyle | **116.685** | 100 % |
| `styleInputOf` on a `colourBy` change (the colours memo misses) | **7.008** | — |
| `styleInputOf`, `sizeBy: "degree"`, first call | **50.029** | — |
| `styleInputOf`, `sizeBy: "degree"`, after it, memos hit | **0.567** | — |

Per-round values, in order: cold `4.986, 1.860, 1.535, 1.566, 1.545`; warm `1.289, 0.689, 0.543,
0.689, 0.547`; `styleFrom` `129.110, 117.427, 110.840, 115.207, 112.991`; both `142.728, 123.941,
113.054, 116.685, 113.550`; colour miss `6.661, 6.991, 7.008, 7.146, 7.967`; degree cold `50.029,
69.562, 44.044, 44.623, 69.977`; degree warm `1.151, 0.567, 0.564, 0.565, 0.570`.

**Caveat:** round 1 of every row carries JIT warm-up (4.986 against 1.5–1.9 for the cold row), and
the medians include it, which biases these numbers *against* the change. Node, not the browser: it
has no React commit and no GL upload, so it decomposes the JS and nothing else.

Two consequences the browser table alone hides:

* **The default look barely uses the memos.** `sizeBy: "weight"` already returns `meta.weight` by
  reference (`styleOf.ts:90`), so the weights memo has nothing to save; the whole default-look
  saving is 1.566 → 0.689 ms, **0.877 ms per restyle**.
* **`sizeBy: "degree"` is where it pays.** 50.029 → 0.567 ms, an **88×** reduction and ~49 ms per
  restyle, because that path rebuilds a `Float32Array` over all 1M nodes.

The browser's 244 ms step is 1.5 ms of `styleInputOf`, 115 ms of `styleFrom` on the Node figure,
0.75 ms of `view.setStyle`, and about 127 ms of work this attribution cannot decompose — the React
commit and whatever else the driver's gap covers. **Caveat:** that last term is a subtraction, not
a measurement.

## 5. What the change does not do

- **`styleFrom` still runs per restyle.** `packages/graph-render/src/style.ts:126` is called on
  every step at `pipeline.ts:124`, memo or not: it allocates `Float32Array(1M)` for the radii,
  walks every node through `radiusFor`, then `rankOf` (`style.ts:120`) builds a `Uint32Array(1M)`
  and **sorts** it with a comparator. That is the 115 ms of the 117 ms, and `graph-render` is
  outside this job's editable paths. Until it is addressed, a reveal step cannot get below ~115 ms
  no matter what `styleInputOf` does.
- **The memo does not make a `colourBy` change cheaper.** A new `colourBy` is a new key, so the
  colours memo misses and rebuilds — 7.008 ms at 1M, exactly the work develop did.
- **A `filter` change misses the mask memo** and rebuilds `hiddenOf`, and a new graph misses all
  three. Those are the intended directions: the keys name what each part reads.
- **`sameSource` was not measured.** It moved from `JSON.stringify(a) === JSON.stringify(b)` to
  field-by-field `===` (`pipeline.ts:98`) because a `document` source carries the whole document
  text, so every settings change (`pipeline.ts:246`) built two strings as long as the document to
  compare them. The new code allocates nothing and is O(1) in the document's size instead of
  O(document); no number is claimed for it because the job asked for no measurement of it and the
  old path only ran on a settings change, not on a reveal.

## 6. Tests and gates

| Check | Result |
|---|---|
| `tests/reveal-data-path.test.ts` (the step-2 test, 7 tests) | **RED** with the memos bypassed — the develop behaviour: 0 pass, 7 fail, rc 1. **GREEN** with them on: 7 pass, 0 fail, rc 0 |
| `tests/source-identity.test.ts` (4 tests, `sameSource` per kind and member) | rc 0, 4 pass |
| negative control: drop `&& a.shape === b.shape` from `sameSynthetic` | **RED** — test 2 fails, 3 pass; restored, 4 pass again |
| `scripts/studio.sh check` | **rc 0** — tsc, eslint `--max-warnings 0`, node:test, render tests, vite build |
| `scripts/studio-filters.sh` | **rc 0** |
| `STUDIO_FILTERS_BREAK=1 scripts/studio-filters.sh` | **rc 1** (negative control must fail) |
| `scripts/studio-smoke.sh` | **rc 0** |
| `STUDIO_SMOKE_BREAK=1 scripts/studio-smoke.sh` | **rc 1** (negative control must fail) |

**Caveat:** "RED on develop" is the develop *behaviour* — `memo.read` replaced by `build()` — not
a checkout of develop, because this job may not change git state. The bypass is one function body
in `look/memo.ts`; with it, all seven tests fail on array identity, and restoring it makes them
pass, so the test is pinned to the memos and not to anything else.
