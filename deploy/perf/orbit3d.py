"""The frame time of one orbit drag over a 3D layout at N nodes (a probe, not a gate).

    scripts/studio-probe.sh orbit3d 20000 layout.basic3d.sphere canvas2d [label] [steps]
    GM_GPU=1 scripts/studio-probe.sh orbit3d 200000 layout.basic3d.sphere webgl2 gpu-200k

Build first (scripts/studio.sh build). Prints the renderer, the open's own seconds, then one JSON
line: the p50/p95/max gap between the frames the drag painted, the loop's own CPU time per frame,
the backend that drew, and the page's errors. Writes target/studio-orbit3d/<label>.png.

The drag is driven in the page (deploy/perf/probes/orbit3d.js says why); this file only opens the
graph and reads the answer back through cdp.py. The edges come from the studio's `random` source
(deploy/perf/drivers/hook.js), so every row of docs/measurements/perf-3d-gl.md is the same graph
shape at its size.

Caveat: a force layout at 5 000 nodes or more is not run as asked: the studio scatters it and
settles it live on the 2D particle mesh (graph-studio motor/settle.ts `planRun`), so spring3d past
that size draws no 3D frame and the probe exits 1 with "view.orbit() is null" and the log line
that says which layout ran, rather than a number.
"""
import importlib
import json
import os
import sys
import tempfile
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "nav"))

import nav  # first: it puts the perf gate's CDP client on the path
import cdp
import gpu
import smokecdp

sys.path.insert(0, str(Path(__file__).resolve().parent))
settle = importlib.import_module("settle")  # its screenshot and error read-back

PROBE = open("deploy/perf/probes/orbit3d.js").read()
# The studio's last two log lines: what a refused layout says about itself.
LAST_LOG = """document.querySelector('graph-studio').studio.store.get().log.slice(-2)
  .map((entry) => ({ command: entry.command, ok: entry.ok, message: entry.message }))"""


def run(page, base, plan):
    page.start_watching()
    page.set_viewport(1920, 1080, 1)
    page.navigate(base)
    name = gpu.check(page)
    page.evaluate(open("deploy/perf/drivers/hook.js").read())
    started = time.monotonic()
    page.evaluate(f"window.__perf.open({plan['nodes']}, '{plan['layout']}')", timeout=900)
    print(f"open {plan['nodes']} on {plan['layout']}: {(time.monotonic() - started):.2f} s")
    try:
        measured = page.evaluate(f"({PROBE})({{ steps: {plan['steps']}, dx: 4, warm: 3 }})", timeout=900)
        print(f"orbit renderer={name}", json.dumps(measured, sort_keys=True))
    finally:
        print("log", json.dumps(page.evaluate(LAST_LOG)))
        settle.errors(page)
        settle.shoot(page, plan["out"])


def main():
    nodes, layout, backend = int(sys.argv[1]), sys.argv[2], sys.argv[3]
    label = sys.argv[4] if len(sys.argv) > 4 else "current"
    steps = int(sys.argv[5]) if len(sys.argv) > 5 else 60
    plan = {"nodes": nodes, "layout": layout, "steps": steps, "out": f"target/studio-orbit3d/{label}.png"}
    os.makedirs(os.path.dirname(plan["out"]), exist_ok=True)
    server = nav.serve("app/dist")
    with tempfile.TemporaryDirectory() as profile:
        browser = nav.launch_browser(profile, extra=gpu.chrome_flags())
        try:
            served = f"http://127.0.0.1:{server.server_address[1]}/?backend={backend}"
            run(smokecdp.Watcher(nav.DEBUG_PORT), served, plan)
        except gpu.SoftwareRasteriser as failure:
            print(f"orbit3d: {failure}", file=sys.stderr)
            return 2
        except cdp.CdpError as failure:
            print(f"orbit3d: refused: {failure}", file=sys.stderr)
            return 1
        finally:
            browser.terminate()
            browser.wait(timeout=10)
            server.shutdown()
    return 0


if __name__ == "__main__":
    sys.exit(main())
