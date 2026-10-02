# Job perf-p5c: measure the 1M studio on the host's real GPU, not on SwiftShader

## Why
Every WebGL2 number so far was drawn by SwiftShader, a CPU rasteriser. All browser probes launch
`--enable-unsafe-swiftshader` (`deploy/nav/backend.py:215`, `deploy/perf/open.py:96`,
`deploy/perf/run.py:71`, `deploy/perf/zoom.py:82`) inside `gm-chromium`, and no `docker run` passes
a device. perf-p5b measured `transferToImageBitmap` at ~42 ms a frame. That cost is the GPU flush of
the chunk's draw, which on SwiftShader is CPU rasterisation, so it says little about a user's machine.
The host has a GPU: an AMD Navi 23 (RX 6600), `/dev/dri/card1` and `/dev/dri/renderD128`. Group
`render` is gid 993 and group `video` is gid 44. The 1M goal must be measured where users run it.

## Do
1. `deploy/chromium.Dockerfile`: add the Mesa userspace for AMD (Debian packages: `libgl1-mesa-dri`,
   `libegl1`, `mesa-vulkan-drivers`, `libvulkan1`, and whatever else the probe proves it needs), then
   rebuild with the build line in the file's header. Additive only: the SwiftShader arm must keep
   working unchanged.
2. Add one opt-in, `GM_GPU=1`, in the single place the perf probes build their `docker run` and
   Chromium flags. Find it, don't copy it per script. With it on:
   - docker gets `--device /dev/dri --group-add 993 --group-add 44`;
   - Chromium drops `--enable-unsafe-swiftshader` and takes the flags that select the hardware ANGLE
     backend (try `--use-angle=vulkan` and `--use-angle=gl-egl`, plus `--ignore-gpu-blocklist`).
     Keep the one that a probe proves is hardware, and record why.
3. Every perf probe prints the renderer it drew on, from the `WEBGL_debug_renderer_info` extension's
   `UNMASKED_RENDERER_WEBGL` parameter, in each result line and each report.
   - Under `GM_GPU=1`, a renderer string that contains `SwiftShader` or `llvmpipe` is an error, exit 2:
     a silent fallback is the failure this job exists to prevent.
   - Negative control: `GM_GPU=1` with the device not passed (a `GM_GPU_BREAK=1` knob) must exit
     non-zero.
4. Measure 3 interleaved rounds per arm, at 200 000 and 1 000 000 nodes, SwiftShader arm against GPU
   arm. Paste a table:
   - `deploy/perf/open.py`: the open, and the first frame;
   - `deploy/perf/settle.py`: time to the full still;
   - `deploy/perf/settle-pan.py`: pan p50 and max gap;
   - `deploy/perf/zoom.py`: zoom frame times;
   - `scripts/studio-perf.sh`: worst fps.
5. From the GPU arm's profile, name the top cost at 1M for each of open, settle and pan. Write
   `docs/measurements/perf-p5c.md`: both tables, the renderer strings, and the ranked list of what
   to attack next. Each item cites file:line and the measured share.
   - Do not optimise in this job. The output is the measurement and the next brief's facts.

## Constraints
- The parity gates (`scripts/studio-backend.sh` and its BREAK negctl) stay on SwiftShader: they need a
  rasteriser that is the same on every host. Do not move them.
- No GPU device in CI. The GPU arm is opt-in and host-local; the Python probes must still run without it.
- `Caveat:` line on the renderer check: a renderer string can name hardware while the browser still
  rasterises part of the frame in software (the GPU blocklist, or a feature the backend lacks).

Done when: perf-p5.rows green (`scripts/studio.sh check`, backend and its negctl, smoke and its negctl),
the `GM_GPU=1` renderer line shows the AMD device, the `GM_GPU_BREAK=1` negctl exits non-zero, and the
tables are pasted with every exit code.
