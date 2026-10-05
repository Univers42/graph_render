"""Where opening a real document goes: the open, the first frame, the heap, and a CPU profile of
the page and of every motor worker (a probe, not a gate).

    scripts/studio-probe.sh open-document target/git-plugin/git/git.studio.json webgl2 \
        target/git-plugin/contributor-stats/contributor-stats.studio.json

Build first (scripts/studio.sh build). Each path is read from the repository root (the container
mounts it at /w, which is the working directory). The page fetches it from a second server this
probe starts on 127.0.0.1 and hands it to the studio as one `source.document` dispatch with the
params `name` and `text` — the params the file control sends, so the measured open is the one a
dropped file goes through. `layout.dag.sugiyama` is selected first, on the graph the studio opened
with, and each document is then laid out once by the layout that was asked for.

It prints, for each document: the dispatch's own milliseconds, the milliseconds to the first
animation frame after it, `performance.memory.usedJSHeapSize` on the page and in the worker that
built the graph, the browser's resident memory, the studio's own message (which names the build and
the layout's own milliseconds), and the profiles' top self and inclusive rows. Then, with a second
document, it opens that one and prints the same. A page exception, a page error or a lost target is
printed and the probe exits 1; exit 2 is the harness (a software rasteriser under GM_GPU=1).

WHY the dispatch is not awaited: a new source starts in a NEW worker (graph-studio
`motor/client.ts`, `loadFresh`), and this client reads CDP frames only inside `call`, so an awaited
dispatch would leave the worker that does the work unattached and unsampled — exactly the caveat
`deploy/perf/open.py` records. The dispatch is started and left running; each poll below is a CDP
round trip that drains the events, which attaches the profiler to the new worker within a
millisecond or two of its start and then reports on it.

Caveat: the open's own milliseconds are `performance.now()` around the dispatch, so they carry the
document's fetch, its structured clone to the worker, the wasm module start of a worker that begins
with it, and the profiler's own few percent; nothing here separates those. The wall clock beside
them is this probe's own polling — a check on that number, not a second measurement. Caveat: a 0.5 ms
sampling profiler: self times under a few samples are noise, wasm names are mangled Rust symbols, the
page's JavaScript is minified (find a row in app/dist/assets/<chunk>.js), and a worker is sampled
from a millisecond or two after it starts. Caveat: the heap is the page's `performance.memory`, which
Chromium rounds, never collects to zero and does not have in a worker, so it says nothing of the wasm
heap: the resident memory printed beside it is the only figure here that sees that heap, and it is the
whole browser, processes this probe does not name. Caveat: the cross-origin fetch of the document is
one request a dropped file does not pay, and one run is one run — take medians over rounds and
compare only arms of this probe on the same host.
"""
import functools
import json
import sys
import tempfile
import threading
import time
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "nav"))

import nav  # first: it puts the perf gate's CDP client on the path
import cdp
import gpu
import smokecdp
# The profiling, the frame watch and the row printer are open.py's, imported by name so the builtin
# `open` stays the builtin: one implementation of the sampling arithmetic for both probes.
from open import FIRST_FRAME, WATCH_FRAMES, report, start

ROWS = 20
LAYOUT = "layout.dag.sugiyama"
POLLS = 4000

# The studio's own action, with the file control's own params. The shared driver
# (deploy/perf/drivers/hook.js) has no document opener, so the dispatch the dock's file control
# makes is spelled out here rather than added to a file both probes share. `__gmOpen` is the
# promise the poll below watches; `__gmSettled` is what it leaves behind.
OPEN_DOCUMENT = """(async (name, url) => {
  window.__gmSettled = null;
  const started = performance.now();
  const answer = await fetch(url);
  if (!answer.ok) throw new Error(`the document server refused ${url}: ${answer.status}`);
  const text = await answer.text();
  const studio = document.querySelector('graph-studio')?.studio ?? null;
  if (studio === null) throw new Error('open-document: no <graph-studio> with a studio on the page');
  const entry = await studio.dispatch('source.document', { name, text });
  const done = { ms: Math.round(performance.now() - started), message: entry.ok
    ? entry.message : `REFUSED ${entry.command}: ${entry.message}` };
  window.__gmSettled = done;
})"""
# A refused or thrown open still has to leave `__gmSettled` behind: nothing else watches the page.
CAUGHT = """(error) => {
  window.__gmSettled = { error: String(error?.message ?? error) };
}"""
SETTLED = "(window.__gmSettled === null ? 'running' : JSON.stringify(window.__gmSettled))"
HEAP = """(() => {
  const memory = performance.memory;
  return memory === undefined ? null : Math.round(memory.usedJSHeapSize / 1048576);
})()"""


class Documents(SimpleHTTPRequestHandler):
    """The listed documents and nothing else, cross-origin: the studio is served by another port.

    `deploy/serve.py`'s handler carries the app's cross-origin isolation, which a document on a
    second origin does not need; what it needs instead is CORS (the page is another origin) and a
    permissive CORP (the page is `Cross-Origin-Embedder-Policy: require-corp`).
    """
    def __init__(self, *args, allowed, **kwargs):
        self.allowed = allowed
        super().__init__(*args, **kwargs)

    def do_GET(self):  # noqa: N802 - the base class names it
        if self.path.split("?")[0] not in self.allowed:
            self.send_error(404, "not one of the probe's documents")
            return
        super().do_GET()

    def end_headers(self):
        self.send_header("Access-Control-Allow-Origin", "*")
        self.send_header("Cross-Origin-Resource-Policy", "cross-origin")
        super().end_headers()

    def log_message(self, format, *args):  # noqa: A002 - the base class names it
        pass


def documents(paths):
    """A server for `paths`, and the URL of each: the repo root is served, the list is the door."""
    handler = functools.partial(Documents, allowed={f"/{path}" for path in paths}, directory=".")
    server = ThreadingHTTPServer(("127.0.0.1", 0), handler)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    return server, [f"http://127.0.0.1:{server.server_address[1]}/{path}" for path in paths]


def harvest(page, done):
    """The worker sessions attached since the last call, profiled, minus the ones that went away.

    A source change closes the worker it replaced (`loadFresh` retires it), so a session that has
    detached between two polls is reported and not profiled.
    """
    fresh: list[str] = []
    for event in page.events:
        if event["method"] != "Target.attachedToTarget":
            continue
        session = event["params"].get("sessionId")
        if session in done or session in page.gone:
            continue
        done.add(session)
        fresh.append(session)
    for session in fresh:
        try:
            start(page, session)
            page.session_call(session, "Runtime.enable")
            print(f"worker {session[:6]}: attached, profiling")
        except (cdp.CdpError, smokecdp.Detached):
            print(f"worker {session[:6]}: detached before its profiler started")
    return fresh


def stop_profiles(page, sessions, rows):
    """The page's profile, then each worker's, as top self and inclusive rows."""
    report("main", page.call("Profiler.stop")["profile"], rows)
    for session in sessions:
        try:
            profile = page.session_call(session, "Profiler.stop", timeout=120)["profile"]
        except cdp.CdpError as error:
            # A worker the open retired answers nothing: the session detached, and that is the
            # expected end of the worker that held the previous document, not a failure.
            if "not found" not in str(error) and not isinstance(error, smokecdp.Detached):
                raise
            print(f"== worker {session[:6]}: retired during the open, not profiled")
            continue
        report(f"worker {session[:6]}", profile, rows)


def worker_heaps(page, sessions):
    """`performance.memory` in each worker still attached, in MB; null where it cannot be read."""
    heaps = []
    for session in sessions:
        try:
            heaps.append(page.session_call(session, "Runtime.evaluate", {
                "expression": HEAP, "returnByValue": True})["result"].get("value"))
        except (cdp.CdpError, smokecdp.Detached):
            heaps.append(None)
    return heaps


def first_frame(page):
    """The milliseconds to the first animation frame after the open returned.

    Headless produces no BeginFrames for a page nobody watches, so none would ever arrive and the
    number would read -1 for every arm. One screencast frame asks the compositor for one.
    """
    page.call("Page.startScreencast", {"format": "jpeg", "quality": 1,
                                       "maxWidth": 2, "maxHeight": 2, "everyNthFrame": 1})
    try:
        return page.evaluate(FIRST_FRAME, timeout=30)
    finally:
        page.call("Page.stopScreencast")


def rss_mb():
    """The resident memory of every Chromium process in this container, in MB: (sum, largest).

    `performance.memory` is the page's JavaScript heap and nothing else, so the wasm heap is only
    visible from outside — and this is the probe's own browser, the only one the container runs.
    """
    total = largest = 0
    for entry in Path("/proc").iterdir():
        if not entry.name.isdigit():
            continue
        try:
            cmdline = (entry / "cmdline").read_bytes()
            if b"chromium" not in cmdline:
                continue
            for line in (entry / "status").read_text().splitlines():
                if line.startswith("VmRSS:"):
                    mb = int(line.split()[1]) / 1024
                    total, largest = total + mb, max(largest, mb)
        except OSError:
            continue
    return round(total), round(largest)


def open_one(page, done, name, url, rows):
    """One open, measured and profiled: the dispatch's ms, the first frame, the heap, the rows.

    Returns the studio's own message, or null where the open threw and `name` is at fault.
    """
    page.evaluate(WATCH_FRAMES)
    start(page)
    # `void`, so the expression's value is undefined and never the promise: the shared client always
    # awaits what it is handed, and an awaited open would put this probe back where open.py starts.
    page.evaluate(f"void (window.__gmOpen = {OPEN_DOCUMENT}"
                  f"({json.dumps(name)}, {json.dumps(url)}).catch({CAUGHT}))")
    began, settled, polls = time.monotonic(), None, 0
    for polls in range(1, POLLS + 1):
        harvest(page, done)
        answer = page.evaluate(SETTLED, timeout=600)
        if answer != "running":
            settled = json.loads(answer)
            break
        time.sleep(0.005)
    live = [session for session in done if session not in page.gone]
    if settled is None:
        print(f"open-document: {name} was still running after {POLLS} polls")
        return None
    if "error" in settled:
        print(f"open {name} failed after {round(time.monotonic() - began, 2)} s: {settled['error']}")
        stop_profiles(page, live, rows)
        return None
    print(f"open {name} ms {settled['ms']} wall s {round(time.monotonic() - began, 2)} "
          f"polls {polls} · {settled['message']}")
    print("first frame ms", first_frame(page))
    print("page heap MB", page.evaluate(HEAP), "worker heap MB", worker_heaps(page, live),
          "chromium rss MB (sum, largest)", rss_mb())
    stop_profiles(page, live, rows)
    return settled["message"]


def faults(page):
    """What the page and its workers threw or logged as errors, one line each."""
    lines = []
    for event in page.events:
        if event["method"] == "Runtime.exceptionThrown":
            detail = event["params"]["exceptionDetails"]
            lines.append(f"exception {detail.get('exception', {}).get('description') or detail.get('text')}")
        if event["method"] == "Log.entryAdded" and event["params"]["entry"]["level"] == "error":
            lines.append(f"log {event['params']['entry']['text']}")
    return lines


def run(page, served, pairs, rows):
    page.set_viewport(1920, 1080, 1)
    page.call("Runtime.enable")
    page.call("Log.enable")
    page.navigate(served)
    gpu.check(page)
    page.evaluate(open("deploy/perf/drivers/hook.js").read())
    # The layout first, on the small graph the studio opened with, so each document is laid out
    # once, by the layout that was asked for (the shared driver's own `open` does the same).
    page.evaluate(f"window.__perf.run({json.dumps(LAYOUT)})", timeout=300)
    done = set()
    opened = [open_one(page, done, name, url, rows) for name, url in pairs]
    for line in faults(page):
        print(line)
    return all(message is not None for message in opened)


def main():
    if len(sys.argv) not in (3, 4):
        print(__doc__.split("\n", 1)[0], file=sys.stderr)
        return 2
    paths = [sys.argv[1], *sys.argv[3:4]]
    for path in paths:
        if not Path(path).is_file():
            print(f"open-document: no such document: {path}", file=sys.stderr)
            return 2
    server = nav.serve("app/dist")
    docs, urls = documents(paths)
    with tempfile.TemporaryDirectory() as profile:
        browser = nav.launch_browser(profile, extra=gpu.chrome_flags())
        try:
            watched = smokecdp.Watcher(nav.DEBUG_PORT)
            watched.call("Target.setAutoAttach", {"autoAttach": True, "waitForDebuggerOnStart": False,
                                                  "flatten": True})
            base = f"http://127.0.0.1:{server.server_address[1]}/?backend={sys.argv[2]}"
            opened = run(watched, base, list(zip(paths, urls)), ROWS)
        except gpu.SoftwareRasteriser as failure:
            print(f"open-document: {failure}", file=sys.stderr)
            return 2
        except (cdp.CdpError, OSError) as failure:
            print(f"open-document: the page went away: {failure}", file=sys.stderr)
            return 1
        finally:
            browser.terminate()
            browser.wait(timeout=10)
            server.shutdown()
            docs.shutdown()
    return 0 if opened else 1


if __name__ == "__main__":
    sys.exit(main())