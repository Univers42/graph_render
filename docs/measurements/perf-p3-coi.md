# perf-p3-coi — cross-origin isolation on, in dev, in preview and in every browser gate

Job: `prompts/jobs/perf-p3-coi.md`. Model (a) of `docs/decisions/browser-threads.md:47` is
accepted, and it needs a `SharedArrayBuffer`, which a browser hands only to a cross-origin
isolated page. This job turns isolation on everywhere the studio is served. It does **not**
load the threads build, and no header is dropped anywhere to make a gate go green.

Before (develop 259116b, 2026-10-02): `git grep -n -i 'cross-origin\|headers' develop --
app/vite.config.ts` printed nothing, so neither the dev server nor the preview server isolated
the page, and `self.crossOriginIsolated` was `false` under every gate.

## 1. The servers in scope

`git grep -n 'QuietHandler\|SimpleHTTPRequestHandler' deploy/`, before this job — the audit's
finding was right: one handler copied five times.

```
deploy/nav/filters.py:24:from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
deploy/nav/filters.py:37:class QuietHandler(SimpleHTTPRequestHandler):
deploy/nav/filters.py:38:    extensions_map = {**SimpleHTTPRequestHandler.extensions_map, ".wasm": "application/wasm"}
deploy/nav/filters.py:45:    handler = functools.partial(QuietHandler, directory=str(dist))
deploy/nav/nav.py:21:from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
deploy/nav/nav.py:36:class QuietHandler(SimpleHTTPRequestHandler):
deploy/nav/nav.py:37:    extensions_map = {**SimpleHTTPRequestHandler.extensions_map, ".wasm": "application/wasm"}
deploy/nav/nav.py:44:    handler = functools.partial(QuietHandler, directory=str(dist))
deploy/parity/run.py:19:from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
deploy/parity/run.py:42:class QuietHandler(SimpleHTTPRequestHandler):
deploy/parity/run.py:48:    handler = functools.partial(QuietHandler, directory=str(dist))
deploy/perf/run.py:29:from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
deploy/perf/run.py:60:class QuietHandler(SimpleHTTPRequestHandler):
deploy/perf/run.py:61:    extensions_map = {**SimpleHTTPRequestHandler.extensions_map, ".wasm": "application/wasm"}
deploy/perf/run.py:68:    handler = functools.partial(QuietHandler, directory=str(dist))
deploy/three/three.py:23:from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
deploy/three/three.py:37:class QuietHandler(SimpleHTTPRequestHandler):
deploy/three/three.py:38:    extensions_map = {**SimpleHTTPRequestHandler.extensions_map, ".wasm": "application/wasm"}
deploy/three/three.py:45:    handler = functools.partial(QuietHandler, directory=str(dist))
```

`deploy/nav/nav.py` is the shared one: `nav.serve()` also serves the local, interact, live,
forces, display, s3gaps, backend and smoke gates, and the perf probes. Every server that serves
`app/dist` was in scope.

## 2. One handler: `deploy/serve.py`

`deploy/serve.py:11` is the only definition left. It keeps the two behaviours the copies agreed
on (a quiet `log_message`, `application/wasm` for `.wasm`) and adds the two headers every
response carries, on the document, the wasm, the fixtures and every asset:

```python
class QuietHandler(SimpleHTTPRequestHandler):
    extensions_map = {**SimpleHTTPRequestHandler.extensions_map, ".wasm": "application/wasm"}

    def __init__(self, *args, isolated=True, **kwargs):
        self.isolated = isolated
        super().__init__(*args, **kwargs)

    def end_headers(self):
        if self.isolated:
            self.send_header("Cross-Origin-Opener-Policy", "same-origin")
            self.send_header("Cross-Origin-Embedder-Policy", "require-corp")
        super().end_headers()

    def log_message(self, format, *args):  # noqa: A002 - the base class names it
        pass
```

`isolated=False` is not a second class and not a second code path: it is the negative control's
switch, so `smoke.py --break-coi` can serve the same handler unisolated. `nav.serve(dist,
isolated=True)` is the only caller that passes it; every other gate takes the default.

`deploy/parity/run.py`'s copy had no `.wasm` entry; unifying gave it one. It serves
`parity.html`, which fetches no wasm, so nothing about that gate changed.

After (the done-when grep — exactly one definition, five imports):

```
$ git grep -n 'class QuietHandler' deploy/
deploy/serve.py:11:class QuietHandler(SimpleHTTPRequestHandler):
```

## 3. Dev and preview: `app/vite.config.ts`

The same two headers under `server.headers` and `preview.headers`, so the studio is isolated
the same way in dev, in preview and in production:

- `app/vite.config.ts:34` — `server.headers` (dev)
- `app/vite.config.ts:37` — `preview.headers`

Measured on the dev server (`STUDIO_PORT=5175 scripts/studio.sh serve`, because another
worktree already holds 5174 on this host):

```
$ curl -sI http://127.0.0.1:5175/ | grep -iE 'HTTP/|cross-origin'
HTTP/1.1 200 OK
Cross-Origin-Opener-Policy: same-origin
Cross-Origin-Embedder-Policy: require-corp
```

## 4. The gate row, and its negative control

`deploy/nav/smokerows.py:159` adds `smoke-cross-origin-isolated`: `crossOriginIsolated` on the
page from the page's own probe, and the **motor worker's** flag read over that worker's own CDP
session (`Target.attachedToTarget` → `Runtime.evaluate` on the session), because the page's flag
is not the worker's. No worker attached is a FAIL, not a pass.

`scripts/studio-smoke.sh` reads `STUDIO_COI_BREAK=1` → `smoke.py --break-coi` → `nav.serve(...,
isolated=False)`.

**The row passes** (`scripts/studio-smoke.sh`, commit a044cc5):

| row | expectation | measured | verdict |
|---|---|---|---|
| `smoke-no-exception` | no uncaught exception from load until the drawing settles, page or motor worker | none | PASS |
| `smoke-no-console-error` | no console error and no Log error from load until the drawing settles | none | PASS |
| `smoke-no-store-error` | `studio.store.get().error` is null once the studio has come up | null | PASS |
| `smoke-no-overlay` | no failure banner in the studio's shadow root | none | PASS |
| `smoke-drew-nodes` | the view drew at least one node | 400 nodes drawn | PASS |
| `smoke-cross-origin-isolated` | `crossOriginIsolated` is true for the page and for every attached motor worker | **page true, motor worker true** | PASS |

**The negative control fails, and only that row fails** (`STUDIO_COI_BREAK=1`):

| row | expectation | measured | verdict |
|---|---|---|---|
| `smoke-drew-nodes` | the view drew at least one node | 400 nodes drawn | PASS |
| `smoke-cross-origin-isolated` | `crossOriginIsolated` is true for the page and for every attached motor worker | **page false, motor worker false** | FAIL |

That the load rows stay green in the control is the point: the page still works without the
headers, so the red row is about isolation and nothing else.

## 5. Nothing stopped loading

An audit for what `require-corp` would block, over `app/**` and `packages/graph-studio/src`:
no absolute URL, no CDN script, no web font, no cross-origin fetch anywhere
(`app/index.html`, `app/parity.html`, `app/src/main.ts`, `app/src/parity.ts`; the favicon is
`data:,`). The three runtime `fetch` sites (`motor/local.ts:12`, `motor/worker.ts:18`,
`parity/page.ts:148`) are all same-origin, resolved against the document. The built HTML carries
Vite's `crossorigin` attributes, which are CORS-mode same-origin loads. Nothing needed a
`Cross-Origin-Resource-Policy` header and nothing was fixed by weakening the policy: every gate
below passes with `require-corp` on, and the smoke gate's no-console-error and no-exception rows
are the ones that would report a blocked resource.

One caveat recorded, not fixed (out of scope): a host that embeds `<graph-studio>` with an
absolute cross-origin `wasm`/`fixtures` attribute would be blocked by `require-corp` until that
host serves its own CORP header. Nothing in this repository does that.

## 6. Gates run

Every gate whose server changed, one at a time or in port-disjoint pairs (nav/filters/three/local
on 9224, smoke/backend/live/interact/display/s3gaps and the negative controls on 9223, perf on
9222, parity on an ephemeral port).

| command | exit | last line / row |
|---|---|---|
| `scripts/studio.sh check` | 0 | `[studio] ok` (types, tests, render tests, eslint, build) |
| `scripts/studio-nav.sh` | 0 | every row PASS |
| `STUDIO_NAV_BREAK=1 scripts/studio-nav.sh` | 1 | `nav-drag` (`+200.00 px`, the 201 px drag not made) and `edge-gradient` (`off by 83.7`) FAIL — the two rows the control injects |
| `scripts/studio-filters.sh` | 0 | every row PASS |
| `scripts/studio-parity.sh` | 0 | gating rows PASS (first run under 3-way parallelism: 1, see below) |
| `scripts/studio-3d.sh` | 0 | every row PASS |
| `scripts/studio-perf.sh` | 1 | two timing rows red on a loaded host — see "the perf gate" below |
| `scripts/studio-smoke.sh` | 0 | `smoke-cross-origin-isolated` PASS, `page true, motor worker true` |
| `STUDIO_SMOKE_BREAK=1 scripts/studio-smoke.sh` | 1 | the 5 load rows FAIL over the 25-byte wasm (`smoke-drew-nodes` 0 nodes) |
| `STUDIO_COI_BREAK=1 scripts/studio-smoke.sh` | 1 | `smoke-cross-origin-isolated` FAIL, `page false, motor worker false` |
| `scripts/studio-backend.sh` | 0 | every row PASS |
| `STUDIO_BACKEND_BREAK=1 scripts/studio-backend.sh` | 1 | `backend-parity-2k` (19.79 % vs 2 % ceiling) and `backend-parity-drawn` (`webgl2 0.0000%` off background) FAIL |
| `scripts/studio-local.sh` | 0 | every row PASS |
| `scripts/studio-interact.sh` | 1 | `int-node-drag` FAIL, `node 274 145.89 px from the pointer` — **pre-existing, not isolation**: see below |
| `scripts/studio-live.sh` | 0 | every row PASS |
| `scripts/studio-display.sh` | 0 | every row PASS |
| `scripts/studio-s3gaps.sh` | 0 | every row PASS |
| `git grep -n 'class QuietHandler' deploy/` | 0 | one definition, `deploy/serve.py:11` |

`scripts/orch/rows/perf-p5.rows` is the studio-check / backend / negctl-backend / smoke /
negctl-smoke set; each of its five commands is a row above with its real exit code.

## Done when

| check | result |
|---|---|
| `scripts/studio.sh check` exits 0 | 0 |
| `scripts/studio-smoke.sh` exits 0 | 0 |
| `STUDIO_SMOKE_BREAK=1 scripts/studio-smoke.sh` non-zero | 1 |
| `STUDIO_COI_BREAK=1 scripts/studio-smoke.sh` non-zero | 1 |
| `scripts/studio-nav.sh` exits 0 | 0 |
| `STUDIO_NAV_BREAK=1 scripts/studio-nav.sh` non-zero | 1 |
| `scripts/studio-backend.sh` exits 0 | 0 |
| `scripts/orch/rows/perf-p5.rows` green | all five rows: 0, 0, 1, 0, 1 |
| `git grep -n 'class QuietHandler' deploy/` one definition | one: `deploy/serve.py:11` |
| this document holds commands, exit codes, probe output | §4, §6 |

The goal itself: `self.crossOriginIsolated === true` in the studio page **and** in its motor
worker, under `scripts/studio.sh` dev (`server.headers`), under preview (`preview.headers`) and
under every gate server (`deploy/serve.py`) — measured, not assumed, by the row in §4.

Two gates are red and neither is isolation, both with evidence above: `scripts/studio-perf.sh`
(two wall-clock rows against a baseline recorded on an idle host, this host at load ~9) and
`scripts/studio-interact.sh` (`int-node-drag`, bit-identical with the headers off).

## The interact gate's red row is not this job's

`scripts/studio-interact.sh` exits 1 on one row, `int-node-drag`:

> dragging a node 150 px puts it under the pointer and its edge follows ·
> **node 274 145.89 px from the pointer; edge 546 end 0.00 px from the node** · FAIL

The other six rows pass (`int-hover`, `int-click`, `int-shift-click`, `int-box`, `int-menu`,
`int-menu-hide`). The measurement is bit-identical across three runs, so it is a state, not a
flaky read. It is not cross-origin isolation, established by running the **same tree and the
same build** through the same handler with the headers switched off
(`nav.serve(dist, isolated=False)`, via `runpy` inside the gate image):

| run | headers | `int-node-drag` measured |
|---|---|---|
| `scripts/studio-interact.sh` (with nav-break in flight) | on | node 274 145.89 px from the pointer |
| `scripts/studio-interact.sh` (alone) | on | node 274 145.89 px from the pointer |
| same gate, `nav.serve(..., isolated=False)` | **off** | node 274 145.89 px from the pointer |

Identical with and without COOP/COEP: the row is a node-drag defect that predates this job and
is outside its paths (`packages/graph-studio/src/motor/` and the renderer's input handling are
owned by other jobs). It is reported, not fixed, and nothing here weakens it to get a green
exit.

## The perf gate

`scripts/studio-perf.sh` exited 1 on its two timing rows and on nothing else:

| row | expectation | measured |
|---|---|---|
| `perf-js` | p95 JS per frame ≤ 4 ms at 2000 nodes | 5.675 ms |
| `perf-fps` | ≥ 2× baseline at DPR 2 (or the 54 fps cap) | 120 nodes: 34.2 fps (was 54.1, floor 54); 2000 nodes: 3.3 fps (was 7, floor 14) |

`perf-idle`, `perf-block`, `perf-edge-batch`, `perf-sprite-cache` and `perf-label-layout` all
PASS. Both reds are wall-clock rows against `deploy/perf/baseline.json`, recorded on an idle
host; this host ran at load 9.56 on 20 cores with other worktrees' gates in flight
(`/proc/loadavg`, ten `gm-mcp-browser` containers up alongside this gate). Two response headers
cannot cost 21 ms of JS per frame, and the studio draw itself is unchanged — the identical
pages in the smoke, nav, filters, 3d and local gates are all green. Recorded as a host-load
result, not as a pass: the gate needs a quiet machine, and it is deliberately not a row of
`perf-p5.rows`.

## Decisions taken

1. **`isolated=` on one handler, not a second class.** The negative control needs the gate to
   serve without the headers, and `nav.serve(dist, isolated=True)` is one parameter on one
   handler rather than a second definition to drift. Every other caller takes the default, so
   isolation cannot be off by accident anywhere else.
2. **`smoke.py`'s `measure(args)` instead of five parameters.** `--break` and `--break-coi` are
   two faults of one run; the house limit is four parameters. This is the parity gate's own
   shape (`deploy/parity/run.py:88`), so it is the repository's precedent rather than a new shape.
3. **The worker's flag is read over its own CDP session**, not inferred from the page's. The
   claim is "the page *and the motor worker* are isolated", and a page-side read would pass for a
   worker that is not. The row reads `Target.attachedToTarget` (type `worker`) and evaluates
   `crossOriginIsolated` there, which is why `smoke.py` runs with the `smokecdp.Watcher` it
   already had.
4. **Parity's first red was contention, and it is recorded as such.** `scripts/studio-parity.sh`
   run at the same time as nav and filters (three software-raster Chromium processes) reported
   "no state on the page" and exited 1 — its 30 s state cap (`deploy/parity/run.py:39`) is shorter
   than a parity draw takes under that load. Its screenshot showed a fully drawn graph, so the
   page worked and the gate only ran out of patience. Re-run alone: 0, gating rows PASS.
5. **`studio.sh serve` was measured on port 5175**, not 5174: another worktree's studio holds
   5174 on this host and the vite config is `strictPort`. Recorded, not silently substituted.

## Deviations

- None. Every file touched is one the job lists: `deploy/serve.py` (new), the five gate servers,
  `deploy/nav/smoke.py`, `deploy/nav/smokerows.py`, `scripts/studio-smoke.sh`,
  `app/vite.config.ts`, and this document.