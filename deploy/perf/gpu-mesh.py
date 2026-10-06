"""Runs the particle mesh's charge pass on a WebGPU device and grades it against the CPU.

    scripts/studio-probe.sh gpu-mesh hardware target/gpu-fixtures
    scripts/studio-probe.sh gpu-mesh software target/gpu-fixtures --only 1k,10k,50k
    GM_GPU=1 scripts/studio-probe.sh gpu-mesh hardware target/gpu-fixtures --only 1k --break butterfly

Build first (scripts/studio.sh build). Serves the repository root on 127.0.0.1 — a secure
context, which WebGPU requires — writes a one-line probe page into target/gpu-js, opens it in
headless Chromium and asks it to run the charge pass over every fixture in the directory that
`--only` keeps.

The page fetches each `.gmfx` over the harness's own origin and passes the `ArrayBuffer` to
`runCharge`, which is `crates/graph-sdk-js/src/gpu/charge.ts` compiled to `target/gpu-js` by the
`gpu-js` gate row. The fixture never crosses as base64: at 1M the file is 100.7 MiB and a
base64 handoff is a 134 MiB string through `import()` (`fixtures/gpu/README.md:188-191`).

Per fixture one line, `PASS <name> …` or `FAIL <name> <failures> …`, with every report field.
The arm is held to its own measured ceiling row in `bounds.ts`, and the guards are the derived
ones: `rmsRel ≤ 1e-4` at every fixture, and at the two 1M fixtures
`maxAbs ≤ |charge| · (2⁻¹¹/√3) / h²` with `h` from the fixture header.

Exit: 0 every fixture passed · 3 any fixture failed, any refusal, or a limit the pass needs is
below what the device reports · 2 the harness could not run. The software refusal is the
analogue of gpu.py's SoftwareRasteriser: under `hardware`, an adapter whose info names
SwiftShader, llvmpipe or lavapipe, or that is a fallback adapter, is a software adapter, and
saying so with exit 3 is what turns GM_GPU_BREAK=1 red — the check stays on and the device is
gone, so the browser must not be believed.

Caveat: the flag sets are Chromium's documented switches on this image's build, tried on one
host's driver stack; a set that works here is not a property of the flag. The measured ceilings
are one device's numbers on one driver stack, and a driver update re-measures them rather than
widening them.
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
from gpu_mesh_page import ASK_JS, PAGE

# The sizes `--only` accepts, as the fixture names' middle word. `1m` is the 1M pair.
SIZES = ("1k", "10k", "50k", "1m")
# A renderer that names one of these computed on the CPU, whatever else the info says. lavapipe
# is Mesa's software Vulkan and llvmpipe its software GL; SwiftShader is the browser's own.
SOFTWARE_MARKS = gpu.SOFTWARE_NAMES + ("lavapipe",)


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

    Chromium writes into the profile as it shuts down, so its removal can lose that race and
    raise `Directory not empty` — which would turn a probe that answered into exit 2, a harness
    failure the harness did not have. Nothing here is worth keeping, so the removal is
    best-effort: the directory is left under /tmp for the session rather than turned into a
    false refusal.
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


def refusal(arm, report):
    """Why this arm's adapter is not the one asked for, or None when it is.

    The analogue of gpu.check's software refusal: an adapter that names a CPU rasteriser, or
    that says it is a fallback, is not the GPU the hardware arm asked for, and reporting its
    number as one is the failure GM_GPU_BREAK=1 exists to catch.
    """
    if arm != "hardware":
        return None
    marks = report["marks"].lower()
    named = [mark for mark in SOFTWARE_MARKS if mark in marks]
    if not named and not report["fallback"]:
        return None
    return (f"asked for the GPU and got a software adapter: "
            f"{', '.join(named) or 'unnameable'}, fallback={report['fallback']}")


def parse_args(argv):
    """`<arm> <dir> [--only 1k,10k,50k,1m] [--break <fault>]`, or a usage error."""
    if len(argv) < 3 or argv[1] not in ("software", "hardware"):
        return None
    arm, directory = argv[1], argv[2]
    only, fault = None, None
    rest = argv[3:]
    while rest:
        head = rest[0]
        if head == "--only" and len(rest) > 1:
            only = rest[1].split(",")
            rest = rest[2:]
        elif head == "--break" and len(rest) > 1:
            fault = rest[1]
            rest = rest[2:]
        else:
            return None
    if only is not None and not all(size in SIZES for size in only):
        return None
    return arm, directory, only, fault


def fixtures(directory, only):
    """Every `mesh-*.gmfx` in `directory` that `--only` keeps, in name order."""
    names = sorted(path.name for path in Path(directory).glob("mesh-*.gmfx"))
    if only is None:
        return names
    return [name for name in names if name.split("-")[1] in only]


def run_fixtures(label, sets, url, names, arm, fault):
    """Reopen the browser on the set that gave the adapter and run every fixture there."""
    flags = next(flags for name, flags in sets if name == label)
    with browser_on(flags) as page:
        page.navigate(url)
        page.evaluate("window.gpuMesh !== undefined", timeout=60)
        failed = False
        for name in names:
            began = time.monotonic()
            try:
                report = page.evaluate(
                    f"window.gpuMesh({name!r}, {arm!r}, {fault!r})", timeout=900)
            except cdp.CdpError as failure:
                print(f"FAIL {name} the page threw: {failure}")
                failed = True
                continue
            ms = time.monotonic() - began
            verdict = "PASS" if report["pass"] else "FAIL"
            if verdict == "FAIL":
                failed = True
            print(f"{verdict} {name} {line(report, ms)}")
        return failed


def line(report, ms):
    """One fixture's line: every report field, and the wall time the harness measured."""
    fields = (
        f"n={report['n']} state={report['state']} side={report['side']} "
        f"rmsAbs={report['rmsAbs']:.6g} rmsRef={report['rmsRef']:.6g} "
        f"rmsRel={report['rmsRel']:.6g} maxAbs={report['maxAbs']:.6g} "
        f"depositedUnits={report['depositedUnits']} repeatEqual={report['repeatEqual']} "
        f"boundsExact={report['boundsExact']} maxAbsGuard={report['maxAbsGuard']:.6g} "
        f"marks={report['marks'] or '(absent)'} fallback={report['fallback']} ms={ms:.1f}"
    )
    return f"{' '.join(report['failures'])} {fields}" if report["failures"] else fields


def write_page():
    """The probe page, written next to the compiled charge pass it imports."""
    target = Path("target/gpu-js")
    target.mkdir(parents=True, exist_ok=True)
    (target / "probe.html").write_text(PAGE)


def main():
    args = parse_args(sys.argv)
    if args is None:
        print("gpu-mesh: usage: gpu-mesh.py software|hardware DIR "
              "[--only 1k,10k,50k,1m] [--break FAULT]", file=sys.stderr)
        return 2
    arm, directory, only, fault = args
    names = fixtures(directory, only)
    if not names:
        print(f"gpu-mesh: no mesh-*.gmfx in {directory} that --only keeps", file=sys.stderr)
        return 2
    sets = candidates(arm)
    server = nav.serve(".")
    url = f"http://127.0.0.1:{server.server_address[1]}/"
    print(f"arm {arm} at {url} over {len(names)} fixture(s)"
          + (f" --only {','.join(only)}" if only else "")
          + (f" --break {fault}" if fault else ""))
    try:
        label, report = first_adapter(sets, url)
        if report is None:
            print("gpu-mesh: the browser ran and no flag set gave an adapter", file=sys.stderr)
            return 3
        refused = refusal(arm, report)
        if refused:
            print(f"gpu-mesh: {refused}", file=sys.stderr)
            return 3
        write_page()
        return 3 if run_fixtures(label, sets, url, names, arm, fault) else 0
    except (cdp.CdpError, OSError) as failure:
        print(f"gpu-mesh: could not run: {failure}", file=sys.stderr)
        return 2
    finally:
        server.shutdown()


if __name__ == "__main__":
    sys.exit(main())
