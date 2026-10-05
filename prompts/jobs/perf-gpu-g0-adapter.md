# Job perf-gpu-g0-adapter (agent build): does the probe browser get a WebGPU adapter, and which

Why: `docs/decisions/gpu-force-tier.md` accepts a WebGPU particle-mesh tier. Every later slice is
gated in `gm-chromium`; if that browser has no WebGPU adapter, the tier cannot be gated and stops.
This job answers that one question with a probe and a measurement file. No product code.

Facts (verified on develop c4c7c8c2, re-verified on ea66e5c7 2026-10-05; a first run on
2026-10-03 did no work, stopped by provider quota):

- `scripts/studio-probe.sh` runs `deploy/perf/<NAME>.py` in the `gm-chromium` image, with the
  worktree at `/w`. `GM_GPU=1` passes `/dev/dri` and the render/video gids (`scripts/orch/gpu.sh`);
  `GM_GPU_BREAK=1` keeps the check but withholds the device (the negative control). It refuses to
  run without `app/dist/index.html` (`scripts/studio-probe.sh:35-38`): build with
  `scripts/studio.sh build`.
- The image is Debian trixie, Chromium 154.0.8037.92, with `mesa-vulkan-drivers` and `libvulkan1`
  (`deploy/chromium.Dockerfile:17-29`; `docker run --rm gm-chromium chromium --version`).
- `deploy/nav/gpu.py` owns the GL flags (`HARDWARE_FLAGS` `:37`, `SOFTWARE_FLAGS` `:39`) and the
  software-renderer refusal (`is_software`, `:82`). It is shared by the parity gates: **do not
  edit it**. Import from it; put any WebGPU flag in the new probe.
- `deploy/perf/open.py:1-30` shows a probe's shape: a docstring that is its manual (with a
  `Caveat:`), `sys.path` onto `deploy/nav`, then `nav`, `gpu`. `deploy/perf/run.py:64` `serve(dist)`
  serves `app/dist` on localhost and `:71` `launch_browser(profile, backend)` launches Chromium.
  The newer probes use `deploy/nav/nav.py:40` `serve(dist)` and `:47` `launch_browser(profile, extra=...)`
  instead (`deploy/perf/open.py` `main`); either pair is fine.
  WebGPU needs a secure context, which `http://127.0.0.1` is.
- The host's GPU is an AMD RX 6600; under `GM_GPU=1` the WebGL2 renderer string names it
  (`deploy/nav/gpu.py:28-36`). WebGPU has never been asked for.

Do, in order:

1. Write `deploy/perf/webgpu.py`, invoked as `scripts/studio-probe.sh webgpu <software|hardware>`:
   - serve `app/dist` (reuse `run.serve`), launch Chromium with the arm's GL flags from `gpu.py`
     plus the WebGPU flags under test (start with `--enable-unsafe-webgpu` and
     `--enable-features=Vulkan`; for `software` also `--use-webgpu-adapter=swiftshader`), open the
     page;
   - evaluate in the page: `'gpu' in navigator`, then `await navigator.gpu.requestAdapter({powerPreference: 'high-performance'})`;
     print `adapter none` or one line each for `info.vendor`, `info.architecture`,
     `info.device`, `info.description`, `isFallbackAdapter` (or `info.isFallbackAdapter`), and the
     limits `maxStorageBufferBindingSize`, `maxBufferSize`, `maxComputeWorkgroupStorageSize`,
     `maxComputeInvocationsPerWorkgroup`, `maxComputeWorkgroupsPerDimension`, and the feature list;
   - then `requestDevice()` and run one trivial compute pass (add 1 to a 1M-element `u32` storage
     buffer, read it back, check every element) and print `compute ok <ms>` or the error;
   - exit 0: an adapter of the asked arm, the compute check held. Exit 3: the browser ran and has
     no adapter, or no device, or the check failed (print why). Exit 2: the harness could not run.
   - Under `hardware`, an adapter whose info names SwiftShader, llvmpipe or lavapipe, or that is a
     fallback adapter, is a software adapter: exit 3 and say so (the analogue of `gpu.py`'s
     refusal). That is what makes `GM_GPU_BREAK=1` turn red.
   - If the first flags give no adapter, try the next documented Chromium/Dawn flags one at a time
     (for example `--enable-features=Vulkan,VulkanFromANGLE`, `--use-angle=vulkan`,
     `--enable-dawn-features=allow_unsafe_apis`, `--ignore-gpu-blocklist`) and record each
     attempt's result. Stop at the first set that gives an adapter.
2. Run the rows in `scripts/orch/rows/gpu-g0.rows` (each row's command is there).
3. Write `docs/measurements/gpu-adapter.md`: the flag sets tried and each one's printed result, per
   arm; the adapter info and limits; the compute check's time; and the answer in one line ("the
   hardware arm gets an AMD adapter under flags X" or "no adapter: <why>"). A `Caveat:` that this is
   one host's driver stack. What it does not do: no force kernel, no throughput number.

Paths you may edit: `deploy/perf/webgpu.py` (new), `docs/measurements/gpu-adapter.md` (new).
Nothing else: not `deploy/nav/`, not the Dockerfile (if the image lacks a package, stop and report
it under decisions needed), not `packages/`, `crates/`, `app/`.

Load-safety: before each probe run, `free -g` shows ≥ 12 GB available and the 1-minute load is
< 14; otherwise wait. One browser probe at a time.

Done when:

- `scripts/orch/gate.sh <logdir> scripts/orch/rows/gpu-g0.rows` → every row PASS
  (`studio-build` 0; both probe rows ran without exit 2; `negctl-hardware` non-zero);
- `docs/measurements/gpu-adapter.md` states the answer for both arms with the printed lines.

Return block:

```
status: done | partial | blocked
answer: hardware=<adapter description | none (why)>; software=<... | none>
flags: <the set that worked, per arm>
changed: <files>
commands: <each row> -> <rc>
deviations: <none | list>
decisions needed: <none | list>
```
