# Job perf-p3-coi (agent build; P3 browser: cross-origin isolation in dev, preview and every browser gate)

Goal: `self.crossOriginIsolated === true` in the studio page and in its motor worker, under
`scripts/studio.sh` (dev and preview) and under every `deploy/` browser gate. Model (a) of
`docs/decisions/browser-threads.md` (one shared `WebAssembly.Memory`) needs it. This job only turns
isolation on. It does not load the threads build.

Facts measured (develop 259116b, 2026-10-02; develop is 5d11a3e on 2026-10-03):
- `app/vite.config.ts` sets no headers: `git grep -n -i 'cross-origin\|headers' develop -- app/vite.config.ts`
  prints nothing.
- `docs/decisions/browser-threads.md:47` accepts model (a). It needs SharedArrayBuffer, which browsers
  expose only to an isolated page.
- An audit found one `QuietHandler` copied into five gate servers: `deploy/nav/filters.py`,
  `deploy/nav/nav.py`, `deploy/parity/run.py`, `deploy/perf/run.py`, `deploy/three/three.py`. Step 1 confirms it.

Do, in order:
1. `git grep -n 'QuietHandler\|SimpleHTTPRequestHandler' deploy/` and paste the list. Every server that
   serves `app/dist` is in scope.
2. Write one handler, `deploy/serve.py`, with quiet logging. Every response carries
   `Cross-Origin-Opener-Policy: same-origin` and `Cross-Origin-Embedder-Policy: require-corp`.
   Replace each copy with an import and delete the copies.
3. In `app/vite.config.ts`, add the same two headers under `server.headers` and `preview.headers`.
4. Add a check to `scripts/studio-smoke.sh` or its probe: the page and the motor worker both report
   `crossOriginIsolated === true`, and the load smoke stays green.
   - Its negative control: `STUDIO_COI_BREAK=1` serves without the headers and must exit non-zero.
5. If a resource stops loading under `require-corp` (a font, a blob worker, a CDN script):
   - name it with file:line;
   - fix it at the source (same origin, or `Cross-Origin-Resource-Policy`);
   - never by dropping the header.
6. Run every browser gate whose server changed, and paste each exit code.

Out of bounds:
- `crates/`, the SDK, `packages/graph-render`.
- `packages/graph-studio/src/motor/` (fix-worker-lost and perf-pm-live own it).
- The threads artifact (perf-p3-browser owns it), and perf-p3-wasm-replicas.
- No new dependency.
- House limits: 40 lines a function, 300 a file, 4 parameters.

Done when:
- `scripts/studio.sh check` exits 0.
- `scripts/studio-smoke.sh` exits 0; `STUDIO_SMOKE_BREAK=1` and `STUDIO_COI_BREAK=1` exit non-zero.
- `scripts/studio-nav.sh` exits 0 and `STUDIO_NAV_BREAK=1` non-zero.
- `scripts/studio-backend.sh` exits 0 and `scripts/orch/rows/perf-p5.rows` is green.
- `git grep -n 'class QuietHandler' deploy/` shows exactly one definition.
- `docs/measurements/perf-p3-coi.md` holds the commands, the exit codes and the probe output.
