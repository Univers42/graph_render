# GPU adapter — does the probe browser hand out a WebGPU adapter, and which

Measured 2026-10-05 on branch `perf-gpu-g0-adapter`, host dlesieur42, image `gm-chromium`
(Chromium 154.0.8037.92, Debian trixie, Mesa Vulkan ICD + libvulkan1; `deploy/chromium.Dockerfile`).
The GPU is an AMD RX 6600 (RADV NAVI23, RDNA-2), reached through the same `--use-angle=vulkan`
WebGL2 arm the perf gates already use (`deploy/nav/gpu.py:37`).

```sh
scripts/studio.sh build
scripts/studio-probe.sh webgpu software                              # software arm
GM_GPU=1 scripts/studio-probe.sh webgpu hardware                     # hardware arm
GM_GPU=1 GM_GPU_BREAK=1 scripts/studio-probe.sh webgpu hardware      # negative control
scripts/orch/gate.sh target/gpu-g0 scripts/orch/rows/gpu-g0.rows     # 4 rows PASS
```

The probe (`deploy/perf/webgpu.py`) serves `app/dist` on `127.0.0.1` — a secure context, which
WebGPU requires — opens it in headless Chromium, and asks the page for
`navigator.gpu.requestAdapter({powerPreference: 'high-performance'})`, then `requestDevice()` and one
compute pass: add 1 to a 1M-element `u32` storage buffer, copy it back, check all 1 000 000 cells,
timed with `performance.now` around the submit and the readback. Flag sets are tried in the order
below, one browser launch each; every set and its result is printed.

**Caveat:** one host's driver stack, one image build, one browser build. A flag set that works here
is a property of this host's Mesa/Vulkan userspace plus this Chromium, not of the flag. The compute
pass is a canary that a device exists and dispatches — it measures **no** throughput and no force
kernel, so `compute ok 8.7 ms` says a dispatch round-trips, not what a tick would cost.

## Answer

**Yes, on both arms.** The hardware arm gets an AMD adapter under the arm's own GL flags plus
`--enable-unsafe-webgpu --enable-features=Vulkan` — nothing further was needed. The software arm gets
a SwiftShader adapter under `gpu.SOFTWARE_FLAGS` plus
`--enable-unsafe-webgpu --use-webgpu-adapter=swiftshader --enable-features=Vulkan`. So G1 can be
gated in this container; `docs/decisions/gpu-force-tier.md` does not stop.

## Flag sets tried, per arm

Only set 1 answered on either arm; sets 2+ are the ladder for a host where it does not, and each was
run at least once in this order-independent check (a temporary reorder, reverted) to confirm the
escalation path works — see "ladder" below.

### hardware (`GM_GPU=1`, `gpu.HARDWARE_FLAGS` + WebGPU flags)

| # | set | flags beyond `HARDWARE_FLAGS` | result |
|---|---|---|---|
| 1 | unsafe webgpu + vulkan | `--enable-unsafe-webgpu --enable-features=Vulkan` | **adapter: amd / rdna-2, not fallback** |
| 2 | unsafe webgpu + VulkanFromANGLE | `… --enable-features=Vulkan,VulkanFromANGLE` | not reached (set 1 answered) |
| 3 | unsafe webgpu + vulkan + dawn | `… --enable-dawn-features=allow_unsafe_apis` | not reached (set 1 answered) |
| 4 | … + no vulkan gl fallback | `… --disable-vulkan-fallback-to-gl-for-testing` | adapter: amd / rdna-2, compute ok 8.7 ms |

### software (no `GM_GPU`, `gpu.SOFTWARE_FLAGS` + WebGPU flags)

| # | set | flags beyond `SOFTWARE_FLAGS` | result |
|---|---|---|---|
| 1 | swiftshader webgpu + vulkan | `--enable-unsafe-webgpu --use-webgpu-adapter=swiftshader --enable-features=Vulkan` | **adapter: google / swiftshader, fallback** |
| 2 | swiftshader through ANGLE too | replaces GL flags: `--enable-unsafe-swiftshader --use-angle=swiftshader` + `--enable-unsafe-webgpu --use-webgpu-adapter=swiftshader` | adapter: google / swiftshader, compute ok 17.1 ms |
| 3 | swiftshader + VulkanFromANGLE + dawn | `--enable-unsafe-webgpu --use-webgpu-adapter=swiftshader --enable-features=Vulkan,VulkanFromANGLE --enable-dawn-features=allow_unsafe_apis` | not reached (set 1 answered) |
| 4 | swiftshader webgpu, `--disable-gpu` dropped | replaces GL flags: `--enable-unsafe-swiftshader` + the same WebGPU flags | not reached |

Note on set 1 of the software arm: it carries `--disable-gpu`, which sounds fatal to a GPU API, and
it is not — Chromium's WebGPU adapter selection runs in the browser process and
`--use-webgpu-adapter=swiftshader` asks that selection for the software adapter explicitly. Set 2
exists because a set that drops `--disable-gpu` is the other way to reach it, and both answer.

### ladder (escalation path, verified by running sets out of order)

The ladder was exercised by temporarily reordering `candidates()` in `deploy/perf/webgpu.py` and
running the same arms, then reverting: hardware set 4 → amd/rdna-2, compute ok 8.7 ms, exit 0;
software set 2 → google/swiftshader, compute ok 17.1 ms, exit 0. So the loop does move past a set
that gives no adapter and does reach a later one, which is the behaviour the ladder exists for.
Those temporary runs are not in the committed logs and are not gate rows.

## The adapters

### hardware: amd / rdna-2 (AMD RX 6600, RADV NAVI23)

```
vendor amd
architecture rdna-2
device (absent)
description (absent)
isFallbackAdapter false
limit maxStorageBufferBindingSize 4294967292
limit maxBufferSize 4294967292
limit maxComputeWorkgroupStorageSize 65536
limit maxComputeInvocationsPerWorkgroup 1024
limit maxComputeWorkgroupsPerDimension 65535
features bgra8unorm-storage,chromium-experimental-multi-draw-indirect,
  chromium-experimental-sampling-resource-table,
  chromium-experimental-timestamp-query-inside-passes,clip-distances,
  core-features-and-limits,depth-clip-control,depth32float-stencil8,dual-source-blending,
  float32-blendable,float32-filterable,indirect-first-instance,primitive-index,
  rg11b10ufloat-renderable,shader-f16,subgroup-size-control,subgroups,
  texture-component-swizzle,texture-compression-bc,texture-compression-bc-sliced-3d,
  texture-compression-unaligned,texture-formats-tier1,texture-formats-tier2,timestamp-query
```

`device` and `description` are absent on this build's adapter info: Chromium exposes vendor and
architecture from the driver's PCI/device id and leaves the rest empty, so a G1 log cannot name a
card by its marketing string from `adapter.info` alone.

The limits G1 needs, all present and comfortably above a particle mesh's working set:
`maxStorageBufferBindingSize` 4 294 967 292 (about 4 GiB, the `u32` maximum minus 4) holds the 1M-node
position and velocity buffers with room to spare; `maxBufferSize` the same;
`maxComputeWorkgroupStorageSize` 65 536 B; `maxComputeInvocationsPerWorkgroup` 1024, so a
`@workgroup_size(256)` mesh stage is fine; `maxComputeWorkgroupsPerDimension` 65 535, so a
`ceil(1M/256)` dispatch (3 907 groups) is fine. `shader-f16` is exposed, and `subgroups` +
`subgroup-size-control`, which the collide stage may want.

### software: google / swiftshader

```
vendor google
architecture swiftshader
device (absent)
description (absent)
isFallbackAdapter true
limit maxStorageBufferBindingSize 1073741824
limit maxBufferSize 1073741824
limit maxComputeWorkgroupStorageSize 32768
limit maxComputeInvocationsPerWorkgroup 256
limit maxComputeWorkgroupsPerDimension 65535
features bgra8unorm-storage,…,texture-compression-astc,…,texture-compression-etc2,…,
  timestamp-query
```

The full feature list is in `target/gpu-g0/probe-software.log`; it differs from the hardware list by
`texture-compression-astc`, `-astc-sliced-3d`, `-etc2` (present) and by `dual-source-blending`,
`primitive-index`, `shader-f16`, `texture-formats-tier1/2`,
`chromium-experimental-multi-draw-indirect` (absent). Two of those matter for G1: **no `shader-f16`**,
so a mesh stage cannot take an f16 fast path here, and
`maxComputeInvocationsPerWorkgroup` is 256, so a software arm caps a stage at
`@workgroup_size(256)` where the hardware arm allows 1024. A kernel written for the hardware arm
must keep its workgroup size ≤ 256 if it is to run on the software arm at all — worth deciding in
G1b, not here.

## The compute check

Add 1 to a 1M-element `u32` storage buffer (`@workgroup_size(64)`, `arrayLength`-guarded), copy the
4 MB back into a `MAP_READ` buffer, check every element, timed around submit + readback:

| arm | time | every cell == 1 |
|---|---|---|
| hardware (amd, rdna-2) | 8.7 ms | yes |
| hardware, SwiftShader path (`GM_GPU_BREAK=1`) | — | refused before the compute pass |
| software (google, swiftshader) | 21.3 ms | yes |

Times are one run each, from the gate's logs. A repeat of the software arm measured 30.4 ms and
17.1 ms on the two flag sets that reached it, so the software number moves by nearly 2x run to run —
it shares the host's CPU with whatever else is running. The hardware numbers were 9.7 ms and 8.7 ms.
Treat all of them as "a dispatch and a 4 MB readback round-tripped in single-digit to tens of
milliseconds", nothing finer. This is **not** a throughput measurement and **not** a force kernel.

## The negative control

`GM_GPU=1 GM_GPU_BREAK=1` keeps the hardware arm's check and withholds `/dev/dri`
(`scripts/orch/gpu.sh`), so `gpu.chrome_flags()` falls back to `SOFTWARE_FLAGS` and the browser
answers with the software adapter:

```
== set 1/4 unsafe webgpu + vulkan
   flags --disable-gpu --enable-unsafe-swiftshader --enable-unsafe-webgpu --enable-features=Vulkan
   vendor google
   architecture swiftshader
   isFallbackAdapter true
   adapter ok: google swiftshader fallback=True
webgpu: asked for the GPU and got a software adapter: swiftshader, fallback=True
```

Exit 3, and row `negctl-hardware` PASSes on its non-zero. The refusal is the analogue of
`gpu.check`'s `SoftwareRasteriser`: under `hardware`, an adapter whose info names `swiftshader`,
`llvmpipe` or `lavapipe`, or that reports `isFallbackAdapter`, is a software adapter and is not
reported as the GPU arm's. `llvmpipe` and `swiftshader` come from `gpu.SOFTWARE_NAMES`, read from
`deploy/nav/gpu.py` rather than copied; `lavapipe` is added here because it is Mesa's software
Vulkan and `gpu.py` never saw one — it only ever read a WebGL2 renderer string.

## Gate

`scripts/orch/gate.sh target/gpu-g0 scripts/orch/rows/gpu-g0.rows`, 2026-10-05:

```
PASS studio-build           exit=0   expect=0       1s
PASS probe-software         exit=0   expect=0       4s
PASS probe-hardware         exit=0   expect=0       3s
PASS negctl-hardware        exit=1   expect=nonzero 2s
```

Both probe rows ran without exit 2, and the negative control is non-zero.

## What this does not do

No force kernel, no throughput number, no port/canary work, nothing about whether the particle mesh
fits these limits in practice. G1b is where the first of those is asked, against the fixtures G1a
emits.