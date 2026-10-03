"""How long the GPU layer takes to fill its settled picture at N open nodes (a probe, not a gate).

    scripts/studio-probe.sh settle 1000000 webgl2 [label] [polls] [every_ms]
    GM_GPU=1 scripts/studio-probe.sh settle 1000000 webgl2 gpu-1m
    scripts/studio-probe.sh settle 200 webgl2 negctl-store 40 50 --break

Build first (scripts/studio.sh build). Prints one line: the milliseconds from the probe's first
poll to the frame whose `view.stats()` says the settled picture holds every edge, the frame times
that poll saw on the way, then the page's errors and the frame's counters. The renderer it drew on
is printed before all of it, and the open's own seconds are printed before that and are not in the
number: the clock starts once the open command has returned. Writes target/studio-settle/<label>.png,
the settled picture itself.

Exit: 0 the store holds no error after the run · 1 it holds one · 2 the harness could not run.

Why the store is a row: this probe used to print the studio's error and exit 0 through it, so a
red `MotorWorkerLost` banner over a graph that drew perfectly was still a green run. `--break` is
that row's negative control: it puts an error into the store through the studio's own `note`, where
every error it holds came from, and the probe must then exit 1. An argv flag rather than an env
var, because `studio-probe.sh` forwards argv into the container and nothing else.

Why the flag and not the counters: `refining` is the loop's own "the still picture still lacks a
chunk" (webgl2/hook.ts), reported in `view.stats()`, so the probe waits on the same signal the
loop asks another frame for. The counters it used to compare (`drawnEdges` against `edges`) say
the same thing a frame later, and read false on a frame that drew no chunk at all.

Caveat: on the software arm the rasteriser is a CPU rasteriser, so the milliseconds rank builds on
this host and are not what a GPU would show; GM_GPU=1 is what measures that, and its number is a
different arm, not a faster one. Either way this host's load average moves them: run it interleaved
with the build it compares against, three times each, and read the medians.
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
import gpu
import smokecdp
import smokerows

SETTLE = open("deploy/perf/probes/settle.js").read()

# `--break` is the negative control for the store row below: it puts an error into the studio's
# store the way the studio's own `note` does, and the probe must then exit 1.
BROKEN = "--break" in sys.argv[1:]
INJECT = "document.querySelector('graph-studio').studio.note('the settle probe negative control')"


def shoot(page, path):
    data = page.call("Page.captureScreenshot", {"format": "png"})["data"]
    with open(path, "wb") as file:
        file.write(base64.b64decode(data))


def errors(page):
    print("exceptions", smokerows.events(page, "Runtime.exceptionThrown"))
    calls = smokerows.events(page, "Runtime.consoleAPICalled")
    print("console errors", [call for call in calls if "error" in call][:5])
    if BROKEN:
        # The negative control: the studio's own `note`, which is how every store error it holds
        # got there, so the row below can be shown to go red for this reason and no other.
        page.evaluate(INJECT)
    probe = page.evaluate(smokerows.PROBE)
    print("store error", probe["error"], "alert", probe.get("alert"))
    return probe["error"]


def run(page, base, nodes, backend, out, polls, every_ms):
    page.start_watching()
    page.set_viewport(1920, 1080, 1)
    page.navigate(base)
    name = gpu.check(page)
    page.evaluate(open("deploy/perf/drivers/hook.js").read())
    started = time.monotonic()
    page.evaluate(f"window.__perf.open({nodes}, 'layout.random')", timeout=600)
    opened = time.monotonic()
    print(f"open {nodes} on layout.random {backend}: {(opened - started):.2f} s")
    measured = page.evaluate(f"({SETTLE})({{ polls: {polls}, everyMs: {every_ms} }})", timeout=polls * every_ms // 1000 + 120)
    print(f"settle renderer={name} {json.dumps(measured, sort_keys=True)}")
    shoot(page, out)
    return errors(page)


def main():
    nodes, backend = int(sys.argv[1]), sys.argv[2]
    label = sys.argv[3] if len(sys.argv) > 3 else "current"
    polls = int(sys.argv[4]) if len(sys.argv) > 4 else 1200
    every_ms = int(sys.argv[5]) if len(sys.argv) > 5 else 50
    out = f"target/studio-settle/{label}.png"
    os.makedirs(os.path.dirname(out), exist_ok=True)
    server = nav.serve("app/dist")
    with tempfile.TemporaryDirectory() as profile:
        browser = nav.launch_browser(profile, extra=gpu.chrome_flags())
        try:
            served = f"http://127.0.0.1:{server.server_address[1]}/?backend={backend}"
            error = run(smokecdp.Watcher(nav.DEBUG_PORT), served, nodes, backend, out, polls, every_ms)
        except gpu.SoftwareRasteriser as failure:
            print(f"settle: {failure}", file=sys.stderr)
            return 2
        finally:
            browser.terminate()
            browser.wait(timeout=10)
            server.shutdown()
    # A store error is the studio saying it is not the state it should be, whatever the numbers
    # above say: a red banner on a graph that drew is not a measurement of that graph.
    return 0 if error is None else 1


if __name__ == "__main__":
    sys.exit(main())