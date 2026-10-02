"""How long the GPU layer takes to fill its settled picture at N open nodes (a probe, not a gate).

    docker run --rm --memory 10g --memory-swap 10g -v "$PWD:/w" -w /w gm-chromium \
      python3 deploy/perf/settle.py 1000000 webgl2 [label]

Build first (scripts/studio.sh build). Prints one line: the milliseconds from the probe's first
poll to the frame whose counters say the picture holds every edge, then the page's errors and the
frame's counters. The open's own seconds are printed before it and are not in the number: the
clock starts once the open command has returned. Writes target/studio-settle/<label>.png, the
settled picture itself.

Why the counters and not a flag: the loop keeps `refining` (hook.ts) and the view's stats
(view-stats.ts) never say it, so the probe reads `drawnEdges` against `edges`, the pairs the
picture holds and the pairs the frame has (webgl2/still.ts).

Caveat: SwiftShader is a CPU rasteriser, so the milliseconds rank builds on this host and are
not what a GPU would show, and this host's load average moves them: run it interleaved with the
build it compares against, three times each, and read the medians.
"""
import base64
import json
import os
import sys
import tempfile
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "nav"))

import nav  # first: it puts the perf gate's CDP client on the path
import smokecdp
import smokerows

SETTLE = open("deploy/perf/probes/settle.js").read()


def shoot(page, path):
    data = page.call("Page.captureScreenshot", {"format": "png"})["data"]
    with open(path, "wb") as file:
        file.write(base64.b64decode(data))


def errors(page):
    print("exceptions", smokerows.events(page, "Runtime.exceptionThrown"))
    calls = smokerows.events(page, "Runtime.consoleAPICalled")
    print("console errors", [call for call in calls if "error" in call][:5])
    probe = page.evaluate(smokerows.PROBE)
    print("store error", probe["error"], "alert", probe.get("alert"))


def run(page, base, nodes, backend, out, polls, every_ms):
    page.start_watching()
    page.set_viewport(1920, 1080, 1)
    page.navigate(base)
    page.evaluate(open("deploy/perf/drivers/hook.js").read())
    started = time.monotonic()
    page.evaluate(f"window.__perf.open({nodes}, 'layout.random')", timeout=600)
    opened = time.monotonic()
    print(f"open {nodes} on layout.random {backend}: {(opened - started):.2f} s")
    measured = page.evaluate(f"({SETTLE})({{ polls: {polls}, everyMs: {every_ms} }})", timeout=polls * every_ms // 1000 + 120)
    print(f"settle {json.dumps(measured, sort_keys=True)}")
    shoot(page, out)
    errors(page)


def main():
    nodes, backend = int(sys.argv[1]), sys.argv[2]
    label = sys.argv[3] if len(sys.argv) > 3 else "current"
    polls = int(sys.argv[4]) if len(sys.argv) > 4 else 1200
    every_ms = int(sys.argv[5]) if len(sys.argv) > 5 else 50
    out = f"target/studio-settle/{label}.png"
    os.makedirs(os.path.dirname(out), exist_ok=True)
    server = nav.serve("app/dist")
    with tempfile.TemporaryDirectory() as profile:
        browser = nav.launch_browser(profile, extra=["--enable-unsafe-swiftshader"])
        try:
            served = f"http://127.0.0.1:{server.server_address[1]}/?backend={backend}"
            run(smokecdp.Watcher(nav.DEBUG_PORT), served, nodes, backend, out, polls, every_ms)
        finally:
            browser.terminate()
            browser.wait(timeout=10)
            server.shutdown()


if __name__ == "__main__":
    main()