"""The frame rate of one camera pan over a filled picture at N open nodes (a probe, not a gate).

    scripts/studio-probe.sh settle-pan 1000000 webgl2 [label] [polls] [every_ms] [measure_ms] [moving_ms]
    GM_GPU=1 scripts/studio-probe.sh settle-pan 1000000 webgl2 gpu-1m

Build first (scripts/studio.sh build). Prints the renderer it drew on, the open's own seconds, then
one line: the milliseconds from the probe's first poll to the first frame of the filled picture, the
frames the pan painted and the p50/p95/max gap between them, split into the pan's own frames (the
gaps ending inside the loop's MOVING_MS) and the re-fill that follows them, then the page's errors.
Writes target/studio-settle-pan/<label>.png, the panned picture itself.

Why in-page rather than CDP input: the pan is the view's own camera move (`view.panBy`, view.ts),
the one a drag makes, so the gaps measure the renderer and not an input round trip — a dispatch
measured through CDP Input carries the browser's own latency in every gap (the same reason
zoom.py drives its wheel events in the page).

Caveat: the gaps count whatever else the page did, so one garbage collection shows in max and p95;
on the software arm the raster is a CPU rasteriser, so the numbers rank builds on one host and are
not what a GPU would show — GM_GPU=1 measures that, as a different arm — so run it interleaved with
the build it compares against and read the medians.
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

PROBE = open("deploy/perf/probes/settle-pan.js").read()


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


def run(page, base, nodes, plan):
    page.start_watching()
    page.set_viewport(1920, 1080, 1)
    page.navigate(base)
    name = gpu.check(page)
    page.evaluate(open("deploy/perf/drivers/hook.js").read())
    started = time.monotonic()
    page.evaluate(f"window.__perf.open({nodes}, 'layout.random')", timeout=600)
    print(f"open {nodes} on layout.random: {(time.monotonic() - started):.2f} s")
    polls, every_ms, measure_ms = plan["polls"], plan["everyMs"], plan["measureMs"]
    timeout = polls * every_ms / 1000 + measure_ms / 1000 + 180
    measured = page.evaluate(
        f"({PROBE})({{ fillPolls: {polls}, everyMs: {every_ms}, measureMs: {measure_ms}, movingMs: {plan['movingMs']} }})",
        timeout=timeout)
    print(f"pan renderer={name}", json.dumps(measured, sort_keys=True))
    errors(page)
    shoot(page, plan["out"])


def main():
    nodes, backend = int(sys.argv[1]), sys.argv[2]
    label = sys.argv[3] if len(sys.argv) > 3 else "current"
    plan = {
        "polls": int(sys.argv[4]) if len(sys.argv) > 4 else 4000,
        "everyMs": int(sys.argv[5]) if len(sys.argv) > 5 else 10,
        "measureMs": int(sys.argv[6]) if len(sys.argv) > 6 else 2000,
        # The loop's own MOVING_MS: the gaps ending inside it are the pan's own frames, the ones
        # after it are the re-fill the pan restarted (canvas2d/loop.ts).
        "movingMs": int(sys.argv[7]) if len(sys.argv) > 7 else 140,
        "out": f"target/studio-settle-pan/{label}.png",
    }
    os.makedirs(os.path.dirname(plan["out"]), exist_ok=True)
    server = nav.serve("app/dist")
    with tempfile.TemporaryDirectory() as profile:
        browser = nav.launch_browser(profile, extra=gpu.chrome_flags())
        try:
            served = f"http://127.0.0.1:{server.server_address[1]}/?backend={backend}"
            run(smokecdp.Watcher(nav.DEBUG_PORT), served, nodes, plan)
        except gpu.SoftwareRasteriser as failure:
            print(f"settle-pan: {failure}", file=sys.stderr)
            return 2
        finally:
            browser.terminate()
            browser.wait(timeout=10)
            server.shutdown()
    return 0


if __name__ == "__main__":
    sys.exit(main())