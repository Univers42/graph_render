"""How fast a large graph settles live: the open, the first live frame, then ticks and draws a second (a probe, not a gate).

    docker run --rm --memory 10g --memory-swap 10g -v "$PWD:/w" -w /w gm-chromium \
      python3 deploy/perf/live-tick.py 400000 webgl2 [seconds] [layout]

Build first (scripts/studio.sh build). Opens N nodes of the perf driver's graph under a force
layout (default layout.forceatlas2.barnes_hut), then watches for `seconds` (default 15). Prints
the open time, the delay from the open's return to the first `force-frame` the worker posts, the
live frames a second (one tick each, `TICKS_PER_FRAME`), the median gap between them, alpha at
the first and last frame, and the page's own animation frames a second over the same window.
LIVE_PROFILE=1 also samples each worker over the window and prints open.py's self/inclusive rows.

A frame is not a tick: a frame after a tick that overran the loop's budget is posted without
stepping (`liveLoop.ts`), so the gap deciles are bimodal and the long mode is the tick.

How it counts: a script installed before the page loads wraps `Worker`, and counts each message
whose body is a `force-frame`. Nothing in the studio is changed to be measured.

Caveat: under SwiftShader the draw shares this host's cores with the motor worker, so the frame
rate is a floor; the tick rate is the worker's own and moves with the host's load average. Run it
three times and read the medians.
"""
import json
import os
import statistics
import sys
import tempfile
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "nav"))

import nav  # first: it puts the perf gate's CDP client on the path
import smokecdp

sys.path.insert(0, str(Path(__file__).resolve().parent))
import importlib
cpu = importlib.import_module("open")  # its profiler start and report, sampled over the live window
PROFILE = os.environ.get("LIVE_PROFILE") == "1"

COUNTER = """
(() => {
  const frames = [];
  const draws = [];
  window.__live = { frames, draws };
  const Base = window.Worker;
  window.Worker = class extends Base {
    constructor(...args) {
      super(...args);
      this.addEventListener("message", (event) => {
        const body = event.data && event.data.body;
        if (body && body.type === "force-frame") frames.push([performance.now(), body.frame.alpha]);
      });
    }
  };
  const tick = (at) => { draws.push(at); requestAnimationFrame(tick); };
  requestAnimationFrame(tick);
})();
"""


def rate(stamps):
    if len(stamps) < 2:
        return 0.0
    return 1000 * (len(stamps) - 1) / (stamps[-1] - stamps[0])


def summary(live, opened_at, window_ms):
    frames = live["frames"]
    after = [f for f in frames if f[0] >= opened_at]
    draws = [d for d in live["draws"] if d >= opened_at]
    gaps = [b[0] - a[0] for a, b in zip(after, after[1:])]
    return {
        "first_frame_ms": round(after[0][0] - opened_at, 1) if after else None,
        "live_frames": len(after),
        "ticks_per_s": round(rate([f[0] for f in after]), 2),
        "median_tick_gap_ms": round(statistics.median(gaps), 1) if gaps else None,
        "tick_gap_deciles_ms": [round(q, 1) for q in statistics.quantiles(gaps, n=10)] if len(gaps) > 1 else None,
        "max_tick_gap_ms": round(max(gaps), 1) if gaps else None,
        "alpha_first": after[0][1] if after else None,
        "alpha_last": after[-1][1] if after else None,
        "draws_per_s": round(rate(draws), 1),
        "window_ms": window_ms,
    }


def run(page, base, nodes, seconds, layout):
    page.call("Page.enable")
    page.call("Target.setAutoAttach", {"autoAttach": True, "waitForDebuggerOnStart": False, "flatten": True})
    page.call("Page.addScriptToEvaluateOnNewDocument", {"source": COUNTER})
    page.set_viewport(1920, 1080, 1)
    page.navigate(base)
    page.evaluate(open("deploy/perf/drivers/hook.js").read())
    began = time.monotonic()
    page.evaluate(f"window.__perf.open({nodes}, {json.dumps(layout)})", timeout=900)
    open_s = round(time.monotonic() - began, 2)
    opened_at = page.evaluate("performance.now()")
    workers = [e["params"]["sessionId"] for e in page.events if e["method"] == "Target.attachedToTarget"]
    if PROFILE:
        for session in workers:
            cpu.start(page, session)
    time.sleep(seconds)
    if PROFILE:
        for session in workers:
            cpu.report(f"worker {session[:6]}", page.session_call(session, "Profiler.stop", timeout=120)["profile"], 25)
    live = page.evaluate("({frames: window.__live.frames, draws: window.__live.draws})")
    errors = page.evaluate("document.querySelector('graph-studio')?.studio?.store.get().error ?? null")
    print(json.dumps({"nodes": nodes, "layout": layout, "open_s": open_s,
                      **summary(live, opened_at, seconds * 1000), "store_error": errors}))


def main():
    nodes, backend = int(sys.argv[1]), sys.argv[2]
    seconds = float(sys.argv[3]) if len(sys.argv) > 3 else 15.0
    layout = sys.argv[4] if len(sys.argv) > 4 else "layout.forceatlas2.barnes_hut"
    server = nav.serve("app/dist")
    with tempfile.TemporaryDirectory() as profile:
        browser = nav.launch_browser(profile, extra=["--enable-unsafe-swiftshader"])
        try:
            served = f"http://127.0.0.1:{server.server_address[1]}/?backend={backend}"
            run(smokecdp.Watcher(nav.DEBUG_PORT), served, nodes, seconds, layout)
        finally:
            browser.terminate()
            browser.wait(timeout=10)
            server.shutdown()


if __name__ == "__main__":
    main()
