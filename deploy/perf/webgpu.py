"""Does the probe browser hand out a WebGPU adapter, and which one (a probe, not a gate).

    scripts/studio-probe.sh webgpu software
    GM_GPU=1 scripts/studio-probe.sh webgpu hardware
    GM_GPU=1 GM_GPU_BREAK=1 scripts/studio-probe.sh webgpu hardware   (the negative control)

Build first (scripts/studio.sh build). Serves app/dist on 127.0.0.1 — a secure context, which
WebGPU requires — opens it in headless Chromium and asks the page for an adapter, one flag set at
a time, in the order below. The first set that returns an adapter is the one everything after it
is printed for; every set tried, with its result, is printed, so the run reads as a table.

Per set: 'gpu' in navigator, then requestAdapter({powerPreference: 'high-performance'}), then
adapter.info (vendor, architecture, device, description), isFallbackAdapter, the limits G1 needs
(maxStorageBufferBindingSize, maxBufferSize, maxComputeWorkgroupStorageSize,
maxComputeInvocationsPerWorkgroup, maxComputeWorkgroupsPerDimension) and the feature list. Then
requestDevice() and one compute pass: add 1 to a 1M-element u32 storage buffer, copy it back and
check every element, timed with performance.now around the submit and the readback.

Exit: 0 an adapter of the asked arm and the compute check held · 3 the browser ran and there was
no adapter, no device, the check failed, or (hardware) the adapter is a software one · 2 the
harness could not run. The software refusal is the analogue of gpu.py's SoftwareRasteriser: under
`hardware`, an adapter whose info names SwiftShader, llvmpipe or lavapipe, or that is a fallback
adapter, is a software adapter, and saying so with exit 3 is what turns GM_GPU_BREAK=1 red — the
check stays on and the device is gone, so the browser must not be believed.

Caveat: the flag sets are Chromium's documented switches on this image's build, tried on one
host's driver stack; a set that works here is not a property of the flag. The compute pass is a
canary that a device exists and dispatches — it measures no throughput, so `compute ok` says
nothing about what a force tick would cost.
"""
import contextlib
import shutil
import sys
import tempfile
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "nav"))

import nav  # first: it puts the perf gate's CDP client on the path
import cdp
import gpu

CELLS = 1000000
# A renderer that names one of these computed on the CPU, whatever else the info says. lavapipe is
# Mesa's software Vulkan and llvmpipe its software GL; SwiftShader is the browser's own. The first
# two are gpu.py's, read from there rather than copied; lavapipe is the Vulkan one gpu.py never saw,
# because it only ever read a WebGL2 renderer string.
SOFTWARE_MARKS = gpu.SOFTWARE_NAMES + ("lavapipe",)
INFO_KEYS = ("vendor", "architecture", "device", "description")
LIMIT_KEYS = ("maxStorageBufferBindingSize", "maxBufferSize", "maxComputeWorkgroupStorageSize",
              "maxComputeInvocationsPerWorkgroup", "maxComputeWorkgroupsPerDimension")

WGSL = """@group(0) @binding(0) var<storage, read_write> cells: array<u32>;
@compute @workgroup_size(64)
fn bump(@builtin(global_invocation_id) id: vec3<u32>) {
  if (id.x < arrayLength(&cells)) { cells[id.x] = cells[id.x] + 1u; }
}"""


def json_array(keys):
    """The tuple as a JavaScript array literal, for the expressions interpolated below."""
    return "[" + ", ".join(f'"{key}"' for key in keys) + "]"


def json_string(text):
    """`text` as a JavaScript string literal: the WGSL goes into the page, not into Python."""
    escaped = text.replace("\\", "\\\\").replace('"', '\\"').replace("\n", "\\n")
    return f'"{escaped}"'


ASK_JS = """(async () => {
  const out = {present: ('gpu' in navigator), adapter: false, fallback: false, marks: ''};
  if (!out.present) { out.why = "no navigator.gpu"; return out; }
  let adapter = null;
  try { adapter = await navigator.gpu.requestAdapter({powerPreference: 'high-performance'}); }
  catch (error) { out.why = 'requestAdapter threw: ' + error; return out; }
  if (!adapter) { out.why = 'adapter none'; return out; }
  out.adapter = true;
  const info = adapter.info || {};
  const parts = [];
  for (const key of %s) {
    const value = info[key];
    parts.push(key + ' ' + (value === undefined || value === '' ? '(absent)' : value));
    if (value) { out.marks = out.marks + ' ' + value; }
  }
  out.lines = parts;
  const fallback = adapter.isFallbackAdapter !== undefined
    ? adapter.isFallbackAdapter : info.isFallbackAdapter;
  out.fallback = fallback === true;
  out.lines.push('isFallbackAdapter ' + fallback);
  for (const key of %s) { out.lines.push('limit ' + key + ' ' + adapter.limits[key]); }
  out.features = Array.from(adapter.features).sort();
  out.lines.push('features ' + out.features.join(','));
  return out;
})()""" % (json_array(INFO_KEYS), json_array(LIMIT_KEYS))

COMPUTE_JS = """(async () => {
  const N = %d, GROUP = 64;
  const adapter = await navigator.gpu.requestAdapter({powerPreference: 'high-performance'});
  if (!adapter) { return {ok: false, why: 'adapter none'}; }
  const device = await adapter.requestDevice();
  device.addEventListener('uncapturederror', (event) => {
    console.error('webgpu', String(event.error && event.error.message));
  });
  const bytes = N * 4;
  const cells = device.createBuffer({size: bytes, usage: GPUBufferUsage.STORAGE
    | GPUBufferUsage.COPY_SRC | GPUBufferUsage.COPY_DST});
  device.queue.writeBuffer(cells, 0, new Uint32Array(N));
  const read = device.createBuffer({size: bytes, usage: GPUBufferUsage.MAP_READ
    | GPUBufferUsage.COPY_DST});
  const module = device.createShaderModule({code: %s});
  const pipeline = device.createComputePipeline({layout: 'auto', compute: {module, entryPoint: 'bump'}});
  const bind = device.createBindGroup({layout: pipeline.getBindGroupLayout(0),
    entries: [{binding: 0, resource: {buffer: cells}}]});
  const began = performance.now();
  const encoder = device.createCommandEncoder();
  const pass = encoder.beginComputePass();
  pass.setPipeline(pipeline);
  pass.setBindGroup(0, bind);
  pass.dispatchWorkgroups(Math.ceil(N / GROUP));
  pass.end();
  encoder.copyBufferToBuffer(cells, 0, read, 0, bytes);
  device.queue.submit([encoder.finish()]);
  await read.mapAsync(GPUMapMode.READ);
  const out = new Uint32Array(read.getMappedRange());
  let bad = 0, first = -1;
  for (let i = 0; i < N; i += 1) { if (out[i] !== 1) { bad += 1; if (first < 0) { first = i; } } }
  read.unmap();
  return {ok: bad === 0, bad, first, ms: performance.now() - began, got: out[0], last: out[N - 1]};
})()""" % (CELLS, json_string(WGSL))


def with_gl(extra, gl=None):
    """`extra` on top of the arm's GL flags from gpu.py, or on top of `gl` where a set has to
    replace them: the software arm's --disable-gpu leaves no GPU process for WebGPU at all."""
    return list(gpu.chrome_flags() if gl is None else gl) + extra


def candidates(arm):
    """The flag sets tried, in order, as (label, complete flags for launch_browser)."""
    unsafe, dawn = "--enable-unsafe-webgpu", "--enable-dawn-features=allow_unsafe_apis"
    if arm == "software":
        swift = [unsafe, "--use-webgpu-adapter=swiftshader"]
        return [
            ("swiftshader webgpu + vulkan", with_gl(swift + ["--enable-features=Vulkan"])),
            ("swiftshader through ANGLE too", with_gl(
                swift + ["--use-angle=swiftshader"],
                gl=["--enable-unsafe-swiftshader", "--use-angle=swiftshader"])),
            ("swiftshader + VulkanFromANGLE + dawn", with_gl(
                swift + ["--enable-features=Vulkan,VulkanFromANGLE", dawn])),
            ("swiftshader webgpu, --disable-gpu dropped", with_gl(
                swift + ["--enable-features=Vulkan", "--enable-unsafe-swiftshader"],
                gl=["--enable-unsafe-swiftshader"])),
        ]
    return [
        ("unsafe webgpu + vulkan", with_gl([unsafe, "--enable-features=Vulkan"])),
        ("unsafe webgpu + VulkanFromANGLE", with_gl(
            [unsafe, "--enable-features=Vulkan,VulkanFromANGLE"])),
        ("unsafe webgpu + vulkan + dawn", with_gl([unsafe, "--enable-features=Vulkan", dawn])),
        ("unsafe webgpu + vulkan + dawn + no vulkan gl fallback", with_gl(
            [unsafe, "--enable-features=Vulkan", dawn,
             "--disable-vulkan-fallback-to-gl-for-testing"])),
    ]


@contextlib.contextmanager
def profile_dir():
    """A fresh Chromium profile directory for one launch, removed on the way out.

    Chromium writes into the profile as it shuts down, so its removal can lose that race and raise
    `Directory not empty` — which would turn a probe that answered into exit 2, a harness failure
    the harness did not have. Nothing here is worth keeping, so the removal is best-effort: the
    directory is left under /tmp for the session rather than turned into a false refusal.
    """
    path = tempfile.mkdtemp()
    try:
        yield path
    finally:
        shutil.rmtree(path, ignore_errors=True)


@contextlib.contextmanager
def browser_on(flags):
    """Chromium on `flags` with the CDP page open on the probe's port, closed on the way out."""
    with profile_dir() as profile:
        browser = nav.launch_browser(profile, extra=flags)
        try:
            yield cdp.Page(nav.DEBUG_PORT)
        finally:
            browser.terminate()
            browser.wait(timeout=10)
            time.sleep(1)


def attempt(flags, url):
    """One browser launch on `flags`, opening `url`: what the page's adapter answer was.

    A fresh browser per set, because the flags are the launch's own and Chromium reads them once.
    """
    with browser_on(flags) as page:
        page.navigate(url)
        return page.evaluate(ASK_JS)


def first_adapter(sets, url):
    """Try each set until one gives an adapter, printing every set and its result as it goes."""
    for number, (label, flags) in enumerate(sets, 1):
        print(f"== set {number}/{len(sets)} {label}\n   flags {' '.join(flags)}")
        try:
            report = attempt(flags, url)
        except (cdp.CdpError, OSError) as failure:
            print(f"   no report: {failure}")
            continue
        for line in report.get("lines", [report.get("why", "nothing reported")]):
            print(f"   {line}")
        if report.get("adapter"):
            print(f"   adapter ok: {report['marks'].strip() or '(info absent)'}"
                  f" fallback={report['fallback']}")
            return label, report
        print("   adapter none")
    return None, None


def run_compute(label, sets, url):
    """Reopen the browser on the set that gave the adapter, and run the compute pass there."""
    flags = next(flags for name, flags in sets if name == label)
    with browser_on(flags) as page:
        page.navigate(url)
        try:
            return page.evaluate(COMPUTE_JS, timeout=300)
        except cdp.CdpError as failure:
            return {"ok": False, "why": str(failure)}


def refusal(arm, report):
    """Why this arm's adapter is not the one asked for, or None when it is.

    The analogue of gpu.check's software refusal: an adapter that names a CPU rasteriser, or that
    says it is a fallback, is not the GPU the hardware arm asked for, and reporting its number as
    one is the failure GM_GPU_BREAK=1 exists to catch.
    """
    if arm != "hardware":
        return None
    marks = report["marks"].lower()
    named = [mark for mark in SOFTWARE_MARKS if mark in marks]
    if not named and not report["fallback"]:
        return None
    return (f"asked for the GPU and got a software adapter: "
            f"{', '.join(named) or 'unnameable'}, fallback={report['fallback']}")


def main():
    arm = sys.argv[1] if len(sys.argv) > 1 else ""
    if arm not in ("software", "hardware"):
        print(f"webgpu: usage: webgpu.py software|hardware, not {arm!r}", file=sys.stderr)
        return 2
    sets = candidates(arm)
    server = nav.serve("app/dist")
    url = f"http://127.0.0.1:{server.server_address[1]}/"
    print(f"arm {arm} at {url}")
    try:
        label, report = first_adapter(sets, url)
        if report is None:
            print("webgpu: the browser ran and no flag set gave an adapter", file=sys.stderr)
            return 3
        refused = refusal(arm, report)
        if refused:
            print(f"webgpu: {refused}", file=sys.stderr)
            return 3
        check = run_compute(label, sets, url)
        print(f"compute ok {check['ms']:.1f} ms over {CELLS} u32" if check.get("ok")
              else f"compute fail {check}")
        if not check.get("ok"):
            print(f"webgpu: {check.get('why') or 'the compute check did not hold'}", file=sys.stderr)
            return 3
        return 0
    except (cdp.CdpError, OSError) as failure:
        print(f"webgpu: could not run: {failure}", file=sys.stderr)
        return 2
    finally:
        server.shutdown()


if __name__ == "__main__":
    sys.exit(main())