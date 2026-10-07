"""Studio GPU-forces gate: the toggle on a built studio, in Chromium on the host's GPU.

Usage: gpuforces.py --dist DIR --out DIR [--commit ID]   (run from this directory, GM_GPU=1)
Exit:  0 every row PASS · 1 a row FAIL or NOT-RUN · 2 the harness could not run

The flags are the gpu-mesh probe's first set (`deploy/perf/gpu-mesh.py`, `candidates`), the one
that opens the hardware adapter on this host. GM_GPU_BREAK=1 keeps the flags and takes the
device away, so the switch falls back to the CPU and the device row must go red.
"""
import json
import sys
import tempfile

import nav  # first: it puts the perf gate's CDP client on the path
import cdp
import gpu
import gpurows
from drive import VIEWPORT, Studio

WEBGPU_FLAGS = ["--enable-unsafe-webgpu", "--enable-features=Vulkan"]


def measure(dist, out, commit):
    server = nav.serve(dist)
    with tempfile.TemporaryDirectory() as profile:
        browser = nav.launch_browser(profile, extra=gpu.chrome_flags() + WEBGPU_FLAGS)
        try:
            page = cdp.Page(nav.DEBUG_PORT)
            studio = Studio(page, f"http://127.0.0.1:{server.server_address[1]}/", wait_for_settle=True)
            studio.open()
            return {"label": out.name, "commit": commit, "break": gpu.broken(),
                    "browser": page.call("Browser.getVersion").get("product"), "viewport": VIEWPORT,
                    "rows": gpurows.run_rows(studio)}
        finally:
            browser.terminate()
            browser.wait(timeout=10)
            server.shutdown()


def main():
    args = nav.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    try:
        report = measure(args.dist, args.out, args.commit)
    except (cdp.CdpError, OSError) as failure:
        print(f"studio-gpu: could not run: {failure}", file=sys.stderr)
        return 2
    (args.out / "report.json").write_text(json.dumps(report, indent=1) + "\n")
    printed = nav.table(report).replace("studio-nav", "studio-gpu", 1).replace("software raster", "host GPU")
    (args.out / "table.md").write_text(printed)
    print(printed)
    return 0 if all(r["verdict"] == "PASS" for r in report["rows"]) else 1


if __name__ == "__main__":
    sys.exit(main())
