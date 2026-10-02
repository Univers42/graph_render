"""Where the time of opening N nodes goes: a CPU profile of the page and its motor worker (a probe, not a gate).

    scripts/studio-probe.sh open 1000000 webgl2 [rows]
    GM_GPU=1 scripts/studio-probe.sh open 1000000 webgl2 20

Build first (scripts/studio.sh build). Opens N nodes of the perf driver's graph with layout.random
and prints the renderer it opened on, the open time and the milliseconds to the first frame drawn
after it, then for the main thread and each worker the self time of the hottest functions and the
inclusive time of every function on the sampled stacks (a function that calls itself counts once
per sample).

Caveat: a 0.5 ms sampling profiler. Self times under a few samples are noise, wasm names are
mangled Rust symbols, and the page's own JavaScript is minified, so a JavaScript row names a
minified function: find it in app/dist/assets/<chunk>.js. Profiling slows the open by a few
percent; compare open times only between runs of this probe, and only between arms — the GPU arm
(profiling off would be fairer, and this probe never turns it off).
"""
import collections
import sys
import tempfile
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "nav"))

import nav  # first: it puts the perf gate's CDP client on the path
import gpu
import smokecdp

INTERVAL_US = 500
# The frame clock, watched from outside the studio: every animation callback's arrival time is
# recorded, so "the first frame after the open returned" is the first arrival past that moment,
# and a frame the open drew while it was still running is not mistaken for it.
WATCH_FRAMES = """(() => {
  const native = window.requestAnimationFrame.bind(window);
  window.__gmFrames = [];
  window.requestAnimationFrame = (callback) => native((time) => {
    window.__gmFrames.push(performance.now());
    callback(time);
  });
})()"""
FIRST_FRAME = """(async () => {
  const done = window.__gmFrames.length;
  const started = performance.now();
  for (let poll = 0; poll < 600 && window.__gmFrames.length <= done; poll += 1) {
    await new Promise((resolve) => setTimeout(resolve, 16));
  }
  const at = window.__gmFrames[done];
  return at === undefined ? -1 : Math.round(at - started);
})()"""


def parents(profile):
    up = {}
    for node in profile["nodes"]:
        for child in node.get("children", []):
            up[child] = node["id"]
    return up


def name(node):
    frame = node["callFrame"]
    return (frame["functionName"] or "(anon)")[:100], frame["url"].rsplit("/", 1)[-1]


def report(title, profile, rows):
    nodes = {node["id"]: node for node in profile["nodes"]}
    up = parents(profile)
    own, inclusive = collections.Counter(), collections.Counter()
    for sample, gap in zip(profile["samples"], profile["timeDeltas"]):
        own[name(nodes[sample])] += gap
        seen, at = set(), sample
        while at is not None:
            key = name(nodes[at])[0]
            if key not in seen:
                seen.add(key)
                inclusive[key] += gap
            at = up.get(at)
    total = sum(own.values()) or 1
    print(f"== {title}: {total / 1e6:.2f} s sampled")
    for (function, url), gap in own.most_common(rows):
        print(f"self {gap / 1000:9.1f} ms {100 * gap / total:5.1f}%  {function} {url}")
    for function, gap in inclusive.most_common(rows * 2):
        print(f"incl {gap / 1000:9.1f} ms  {function}")


def start(page, session=None):
    for method, params in (("Profiler.enable", None),
                           ("Profiler.setSamplingInterval", {"interval": INTERVAL_US}),
                           ("Profiler.start", None)):
        if session is None:
            page.call(method, params)
        else:
            page.session_call(session, method, params)


def run(page, base, nodes, rows):
    page.call("Target.setAutoAttach", {"autoAttach": True, "waitForDebuggerOnStart": False, "flatten": True})
    page.set_viewport(1920, 1080, 1)
    page.navigate(base)
    name = gpu.check(page)
    page.evaluate(open("deploy/perf/drivers/hook.js").read())
    page.evaluate("window.__perf.run('layout.random')")
    workers = [e["params"]["sessionId"] for e in page.events if e["method"] == "Target.attachedToTarget"]
    for session in workers:
        start(page, session)
    start(page)
    page.evaluate(WATCH_FRAMES)
    began = time.monotonic()
    page.evaluate(f"window.__perf.open({nodes}, 'layout.random')", timeout=300)
    print(f"open s {round(time.monotonic() - began, 2)} renderer={name} workers {len(workers)}")
    print("first frame ms", page.evaluate(FIRST_FRAME, timeout=30))
    report("main", page.call("Profiler.stop")["profile"], rows)
    for session in workers:
        report(f"worker {session[:6]}", page.session_call(session, "Profiler.stop", timeout=120)["profile"], rows)


def main():
    nodes, backend = int(sys.argv[1]), sys.argv[2]
    server = nav.serve("app/dist")
    with tempfile.TemporaryDirectory() as profile:
        browser = nav.launch_browser(profile, extra=gpu.chrome_flags())
        try:
            served = f"http://127.0.0.1:{server.server_address[1]}/?backend={backend}"
            run(smokecdp.Watcher(nav.DEBUG_PORT), served, nodes, 20)
        except gpu.SoftwareRasteriser as failure:
            print(f"open: {failure}", file=sys.stderr)
            return 2
        finally:
            browser.terminate()
            browser.wait(timeout=10)
            server.shutdown()
    return 0


if __name__ == "__main__":
    sys.exit(main())
