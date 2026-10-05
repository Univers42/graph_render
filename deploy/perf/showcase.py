"""Record the studio's showcase: a scripted tour, captured frame by frame (a probe, not a gate).

    GM_GPU=1 scripts/studio-probe.sh showcase discover        # catalog + one screenshot
    GM_GPU=1 scripts/studio-probe.sh showcase record [scene]  # every scene, or one by name

Build first (scripts/studio.sh build). `record` writes target/showcase/frames/*.jpg,
target/showcase/timeline.json (each frame's seconds on screen, each caption's start) and the PNG
stills the scenes ask for under target/showcase/stills/; scripts/showcase.sh encodes them. The
scenes are deploy/perf/showcase_scenes.py; every step is a studio action (drivers/showcase.js).

Caveat: the video is a screencast of a headless browser, so its frame rate is what the GPU, the
page and the JPEG encoder kept up with on this host, and a 1M-node load is cut from the recording
rather than shown at its real length. The stills are full screenshots, not screencast frames.
"""
import json
import os
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "nav"))
sys.path.insert(0, str(Path(__file__).resolve().parent))

import nav  # first: it puts the perf gate's CDP client on the path
import gpu
import showcase_scenes
import smokerows
from screencast import Recorder

DRIVER = open("deploy/perf/drivers/showcase.js").read()
OUT = "target/showcase"
WIDTH, HEIGHT = 1920, 1080


def step(page, kind, *args):
    if kind == "run":
        print(" ", args[0], json.dumps(args[1] if len(args) > 1 else {}), flush=True)
        page.evaluate(f"window.__show.run({json.dumps(args[0])}, {json.dumps(args[1] if len(args) > 1 else {})})",
                      timeout=900)
    elif kind == "hold":
        page.hold(args[0])
    elif kind == "glide":
        page.evaluate(f"window.__show.glide({json.dumps(args[0])})", timeout=120)
    elif kind == "caption":
        page.mark(json.dumps({"title": args[0], "sub": args[1] if len(args) > 1 else ""}))
    elif kind == "still":
        page.screenshot(os.path.join(OUT, "stills", f"{args[0]}.png"))
    elif kind == "chrome":
        page.evaluate(f"window.__show.chrome({json.dumps(args[0])})")
    elif kind == "cut":
        page.pause()
    elif kind == "roll":
        page.resume(WIDTH, HEIGHT)
    else:
        raise ValueError(f"showcase: unknown step {kind}")


def errors(page):
    exceptions = smokerows.events(page, "Runtime.exceptionThrown")
    probe = page.evaluate(smokerows.PROBE)
    print("exceptions", exceptions[:3], "store error", probe["error"], "alert", probe.get("alert"))
    return 0 if not exceptions and probe["error"] in (None, "") else 1


def record(page, names):
    page.resume(WIDTH, HEIGHT)
    for name, steps in showcase_scenes.SCENES:
        if names and name not in names:
            continue
        print("scene", name, flush=True)
        for entry in steps:
            step(page, *entry)
    page.pause()
    frames, marks = page.timeline()
    with open(os.path.join(OUT, "timeline.json"), "w") as file:
        json.dump({"frames": frames, "marks": marks}, file)
    seconds = sum(gap for _, gap in frames)
    print("frames", len(frames), "seconds", round(seconds, 1), "fps", round(len(frames) / max(seconds, 1e-9), 1))


def run(page, served, mode, names):
    page.start_watching()
    page.set_viewport(WIDTH, HEIGHT, 1)
    page.navigate(served)
    print("renderer", gpu.check(page))
    page.evaluate(DRIVER)
    page.evaluate("window.__show.run('view.fit')", timeout=120)
    if mode == "discover":
        print(json.dumps(page.evaluate("window.__show.catalog()")))
        page.hold(3.0)
        page.screenshot(os.path.join(OUT, "discover.png"))
    else:
        record(page, names)
    return errors(page)


def main():
    mode, names = (sys.argv[1] if len(sys.argv) > 1 else "discover"), set(sys.argv[2:])
    backend = next((name.split("=", 1)[1] for name in names if name.startswith("backend=")), "webgl2")
    names = {name for name in names if not name.startswith("backend=")}
    os.makedirs(os.path.join(OUT, "stills"), exist_ok=True)
    server = nav.serve("app/dist")
    with tempfile.TemporaryDirectory() as profile:
        browser = nav.launch_browser(profile, extra=gpu.chrome_flags())
        try:
            served = f"http://127.0.0.1:{server.server_address[1]}/?backend={backend}"
            return run(Recorder(nav.DEBUG_PORT, OUT), served, mode, names)
        except gpu.SoftwareRasteriser as failure:
            print(f"showcase: {failure}", file=sys.stderr)
            return 2
        finally:
            browser.terminate()
            browser.wait(timeout=10)
            server.shutdown()


if __name__ == "__main__":
    sys.exit(main())
