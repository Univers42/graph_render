"""Where the milliseconds of the settled picture go: a CPU profile over the fill (a probe, not a gate).

    docker run --rm --memory 10g --memory-swap 10g -v "$PWD:/w" -w /w gm-chromium \
      python3 deploy/perf/settle-profile.py 1000000 webgl2 [label]

Build first (scripts/studio.sh build). Opens N nodes on layout.random, samples the renderer's main
thread once a millisecond while the settled picture fills, and prints the frames whose own samples
cost the most, as a share of the sampled time. The wait is settle.py's: the picture is full when
drawnEdges reaches edges.

Caveat: a sampling profile of a CPU rasteriser names the JavaScript around the GL calls, not the
raster inside the GPU process, so a row reading low here is work the renderer spent waiting on the
driver. What it does name is which of the renderer's own rows owns the fill.
"""
import json
import os
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "nav"))

import nav  # first: it puts the perf gate's CDP client on the path
import smokecdp
import smokerows

SETTLE = open("deploy/perf/probes/settle.js").read()


def rows(profile, limit):
    """Self time per call frame, as a share of the samples: the frames the renderer sat in."""
    total = sum(node.get("hitCount", 0) for node in profile.get("nodes", []))
    cost = {}
    for node in profile.get("nodes", []):
        hits = node.get("hitCount", 0)
        if hits == 0:
            continue
        frame = node.get("callFrame", {})
        name = frame.get("functionName") or "(anonymous)"
        url = frame.get("url", "").rsplit("/", 1)[-1]
        line = frame.get("lineNumber", 0)
        key = f"{name} ({url}:{line})"
        cost[key] = cost.get(key, 0) + hits
    ordered = sorted(cost.items(), key=lambda item: -item[1])
    return total, [(key, hits, 100.0 * hits / total) for key, hits in ordered[:limit]]


def run(page, base, nodes, label, limit):
    page.start_watching()
    page.set_viewport(1920, 1080, 1)
    page.navigate(base)
    page.evaluate(open("deploy/perf/drivers/hook.js").read())
    page.call("Profiler.enable")
    page.call("Profiler.setSamplingInterval", {"interval": 1000})
    page.evaluate(f"window.__perf.open({nodes}, 'layout.random')", timeout=600)
    page.call("Profiler.start")
    measured = page.evaluate(f"({SETTLE})({{ polls: 2000, everyMs: 50 }})", timeout=240)
    profile = page.call("Profiler.stop", timeout=120).get("profile", {})
    page.call("Profiler.disable")
    print(f"settle {json.dumps(measured, sort_keys=True)}")
    total, top = rows(profile, limit)
    print(f"label {label} · samples {total} · about {total / 1000:.1f} s of the renderer's own time")
    for key, hits, share in top:
        print(f"  {share:5.1f}%  {hits:6d}  {key}")
    os.makedirs(f"target/studio-settle-profile", exist_ok=True)
    with open(f"target/studio-settle-profile/{label}.json", "w") as out:
        json.dump(profile, out)


def main():
    nodes, backend = int(sys.argv[1]), sys.argv[2]
    label = sys.argv[3] if len(sys.argv) > 3 else "current"
    limit = int(sys.argv[4]) if len(sys.argv) > 4 else 25
    server = nav.serve("app/dist")
    with tempfile.TemporaryDirectory() as profile:
        browser = nav.launch_browser(profile, extra=["--enable-unsafe-swiftshader"])
        try:
            served = f"http://127.0.0.1:{server.server_address[1]}/?backend={backend}"
            run(smokecdp.Watcher(nav.DEBUG_PORT), served, nodes, label, limit)
        finally:
            browser.terminate()
            browser.wait(timeout=10)
            server.shutdown()


if __name__ == "__main__":
    main()