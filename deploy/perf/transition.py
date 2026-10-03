"""The cost of a layout switch on N nodes, on one backend: a probe, not a gate.

    scripts/studio-probe.sh transition 20000 webgl2
    scripts/studio-probe.sh transition 200000 canvas2d layout.bfs 3

Build first (scripts/studio.sh build). Prints the renderer it drew on, one JSON line per round
with the five timed points, the frame gaps the tween held, and the page's errors, and writes
target/studio-transition/<backend>-<nodes>/settled.png.

The tween is `from` -> `to` -> `from`, ROUNDS times each way, so every round starts from the same
settled picture. `from` defaults to `layout.random` and `to` to `layout.grid`, two layouts every
random graph accepts: a switch that changes the node count is not a transition at all
(`showFrame` snaps those), and a layout the graph refuses leaves no tween to measure.

Caveat: `clickMs` is the probe's own dispatch, not a real pointer event: it times the studio's
action registry, which is the path a dock click takes too, but not the browser's hit testing.
`gapP50`/`gapP95` are the gaps between animation frames over the whole main thread, so a garbage
collection or the snapshot's own decode shows in them unattributed. On the software arm the
rasteriser is a CPU rasteriser, so the numbers rank builds on one host, they are not what a GPU
would show; GM_GPU=1 measures that, as a different arm.
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

PROBE = open("deploy/perf/probes/transition.js").read()
DRIVER = open("deploy/perf/drivers/hook.js").read()
ROUNDS = 3


def shoot(page, out, name):
    data = page.call("Page.captureScreenshot", {"format": "png"})["data"]
    with open(os.path.join(out, f"{name}.png"), "wb") as file:
        file.write(base64.b64decode(data))


def errors(page):
    print("exceptions", smokerows.events(page, "Runtime.exceptionThrown"))
    calls = smokerows.events(page, "Runtime.consoleAPICalled")
    print("console errors", [call for call in calls if "error" in call][:5])
    probe = page.evaluate(smokerows.PROBE)
    print("store error", probe["error"], "alert", probe.get("alert"))


def run(page, served, nodes, out, args, backend):
    page.start_watching()
    page.set_viewport(1920, 1080, 1)
    page.navigate(served)
    print("renderer", gpu.check(page), "backend", backend)
    page.evaluate(DRIVER)
    started = time.monotonic()
    page.evaluate(f"window.__perf.open({nodes}, 'layout.random')", timeout=600)
    print("open s", round(time.monotonic() - started, 2), "layouts", page.evaluate("window.__perf.layouts()"))
    time.sleep(1.0)
    shot = page.evaluate(f"({PROBE})({json.dumps(args)})", timeout=600)
    print("tweened", shot["from"], "->", shot["to"])
    for row in shot["rows"]:
        print(json.dumps(row))
    shoot(page, out, "settled")
    errors(page)


def main():
    nodes, backend = int(sys.argv[1]), sys.argv[2]
    # Two layouts every random graph accepts: a tree layout is refused on one with cycles, and a
    # refused run leaves no tween to measure.
    args = {"from": "layout.random", "to": "layout.grid", "rounds": int(sys.argv[4]) if len(sys.argv) > 4 else ROUNDS}
    if len(sys.argv) > 3:
        args["from"] = sys.argv[3]
    out = os.path.join("target/studio-transition", f"{backend}-{nodes}")
    os.makedirs(out, exist_ok=True)
    server = nav.serve("app/dist")
    with tempfile.TemporaryDirectory() as profile:
        browser = nav.launch_browser(profile, extra=gpu.chrome_flags())
        try:
            served = f"http://127.0.0.1:{server.server_address[1]}/?backend={backend}"
            run(smokecdp.Watcher(nav.DEBUG_PORT), served, nodes, out, args, backend)
        except gpu.SoftwareRasteriser as failure:
            print(f"transition: {failure}", file=sys.stderr)
            return 2
        finally:
            browser.terminate()
            browser.wait(timeout=10)
            server.shutdown()
    return 0


if __name__ == "__main__":
    sys.exit(main())