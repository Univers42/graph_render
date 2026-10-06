"""Open one document under one layout and watch memory until it settles or the tab dies (a probe).

    scripts/studio-probe.sh ingest-file DOC[,DOC...] BACKEND|default LAYOUT|default [watch_s]
    scripts/studio-probe.sh ingest-file target/crash/git-ct.studio.json default layout.force.particle_mesh 60

Throwaway diagnosis probe (branch studio-85k-crash). The document goes in through the studio's own
`source.document` dispatch, the one the dock's "Ingest JSON" file control makes, exactly as
`open-document.py` sends it; the layout is selected first on the opening graph. No CPU profiler:
it would add its own memory. Every second it prints the page heap, the studio's busy list and
error, the Chromium processes' RSS by type, and the container cgroup's memory and OOM kills; every
few seconds the size of every WebAssembly.Memory in each attached worker (CDP queryObjects).

Caveat: queryObjects forces a full GC in that worker, which can hide a garbage peak between two
samples; the RSS line beside it is the unperturbed figure. Caveat: the RSS sum counts shared pages
once per process, so it over-reports; the cgroup's memory.current is the kernel's own count.
"""
import importlib.util
import json
import os
import sys
import tempfile
import threading
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "nav"))
sys.path.insert(0, str(HERE))
import nav  # noqa: E402 - first: it puts the perf gate's CDP client on the path
import cdp  # noqa: E402
import gpu  # noqa: E402
import smokecdp  # noqa: E402
from open_document_js import CAUGHT, HEAP, OPEN_DOCUMENT, SETTLED  # noqa: E402

spec = importlib.util.spec_from_file_location("open_document", HERE / "open-document.py")
od = importlib.util.module_from_spec(spec)
spec.loader.exec_module(od)

CGROUP = Path("/sys/fs/cgroup")
STATE = """(() => {
  const s = document.querySelector('graph-studio')?.studio?.store.get();
  if (!s) return 'no-studio';
  const m = performance.memory;
  return JSON.stringify({ heap: m ? Math.round(m.usedJSHeapSize / 1048576) : null,
    busy: s.busy.map((b) => b.command ?? b.id ?? b.kind ?? JSON.stringify(b).slice(0, 40)),
    error: s.error ? `${s.error.title}: ${String(s.error.detail).slice(0, 200)}` : null,
    nodes: s.graph?.nodeCount ?? null, run: s.run?.layoutId ?? s.run?.layout ?? null,
    hud: [...(document.querySelector('graph-studio')?.shadowRoot?.querySelectorAll('*') ?? [])]
      .find((e) => e.childElementCount === 0 && / n · /.test(e.textContent ?? ''))?.textContent ?? null });
})()"""
WASM_SIZES = "function () { return this.map((m) => Math.round(m.buffer.byteLength / 1048576)); }"


def cgroup():
    def read(name):
        try:
            return (CGROUP / name).read_text()
        except OSError:
            return ""
    current = read("memory.current").strip()
    peak = read("memory.peak").strip()
    events = dict(line.split() for line in read("memory.events").splitlines() if line)
    mb = lambda v: round(int(v) / 1048576) if v.isdigit() else None  # noqa: E731
    return {"cur": mb(current), "peak": mb(peak), "oom": events.get("oom"), "oom_kill": events.get("oom_kill")}


def processes():
    """RSS in MB of every chromium process, keyed `type:pid`."""
    out = {}
    for entry in Path("/proc").iterdir():
        if not entry.name.isdigit():
            continue
        try:
            cmd = (entry / "cmdline").read_bytes().split(b"\0")
            if not cmd or b"chromium" not in cmd[0]:
                continue
            words = b" ".join(cmd).split()
            kind = next((a[7:].decode() for a in words if a.startswith(b"--type=")), "browser")
            for line in (entry / "status").read_text().splitlines():
                if line.startswith("VmRSS:"):
                    out[f"{kind}:{entry.name}"] = round(int(line.split()[1]) / 1024)
        except OSError:
            continue
    return out


class Sampler(threading.Thread):
    """Proc and cgroup every 0.5 s, independent of the CDP socket: it outlives a dead tab."""

    def __init__(self):
        super().__init__(daemon=True)
        self.peak_by_type, self.last, self.stop = {}, {}, False

    def run(self):
        while not self.stop:
            procs = processes()
            for key, mb in procs.items():
                kind = key.split(":")[0]
                self.peak_by_type[kind] = max(self.peak_by_type.get(kind, 0), mb)
            self.last = {"procs": procs, "cg": cgroup()}
            time.sleep(0.5)


def wasm_sizes(page, session):
    """MB of each WebAssembly.Memory alive in `session`'s worker, or a string saying why not."""
    try:
        proto = page.session_call(session, "Runtime.evaluate",
                                  {"expression": "WebAssembly.Memory.prototype"}, timeout=60)
        arr = page.session_call(session, "Runtime.queryObjects",
                                {"prototypeObjectId": proto["result"]["objectId"]}, timeout=120)
        got = page.session_call(session, "Runtime.callFunctionOn", {
            "functionDeclaration": WASM_SIZES, "objectId": arr["objects"]["objectId"],
            "returnByValue": True}, timeout=60)
        return got["result"].get("value")
    except (cdp.CdpError, smokecdp.Detached, OSError, KeyError) as error:
        return f"unreadable: {str(error)[:80]}"


def sessions_of(page):
    live = []
    for event in page.events:
        if event["method"] == "Target.attachedToTarget":
            session = event["params"]["sessionId"]
            kind = event["params"]["targetInfo"].get("type")
            if session not in page.gone and (session, kind) not in live:
                live.append((session, kind))
    return live


def crashes(page):
    return [e["method"] for e in page.events if "rash" in e["method"] or e["method"] == "Inspector.detached"]


def tick(page, sampler, began, n):
    sample = sampler.last
    procs = sorted(sample.get("procs", {}).items(), key=lambda kv: -kv[1])[:4]
    try:
        state = page.evaluate(STATE, timeout=20)
    except (cdp.CdpError, OSError) as error:
        state = f"EVAL FAILED: {str(error)[:100]}"
    line = f"t={time.monotonic() - began:6.1f}s state={state} cg={sample.get('cg')} top={procs}"
    if n % 5 == 0:
        sizes = [(s[:6], k, wasm_sizes(page, s)) for s, k in sessions_of(page) if k == "worker"]
        line += f" wasm={sizes}"
    print(line, flush=True)
    return state


def watch(page, sampler, name, url, seconds):
    page.evaluate(f"void (window.__gmOpen = {OPEN_DOCUMENT}({json.dumps(name)}, {json.dumps(url)}).catch({CAUGHT}))")
    began, settled_at, n, failures = time.monotonic(), None, 0, 0
    while True:
        n += 1
        for session, kind in sessions_of(page):
            if session not in page.sessions:
                page.sessions.append(session)
                try:
                    page.session_call(session, "Runtime.enable")
                except (cdp.CdpError, smokecdp.Detached):
                    pass
        if settled_at is None:
            try:
                answer = page.evaluate(SETTLED, timeout=20)
                if answer != "running":
                    settled_at = time.monotonic()
                    print(f"OPEN SETTLED after {settled_at - began:.1f} s: {answer}", flush=True)
            except (cdp.CdpError, OSError) as error:
                print(f"settled poll failed: {str(error)[:100]}", flush=True)
        state = tick(page, sampler, began, n)
        failures = failures + 1 if str(state).startswith("EVAL FAILED") else 0
        if failures >= 3 or crashes(page):
            print("TAB GONE:", crashes(page), flush=True)
            return False
        if settled_at is not None and time.monotonic() - settled_at > seconds:
            return True
        time.sleep(1)


def run(page, served, name, url, layout, seconds, sampler):
    page.set_viewport(1920, 1080, 1)
    for domain in ("Runtime", "Log", "Inspector"):
        page.call(f"{domain}.enable")
    page.navigate(served)
    page.evaluate(open("deploy/perf/drivers/hook.js").read())
    if layout != "default":
        page.evaluate(f"window.__perf.run({json.dumps(layout)})", timeout=300)
    print("layout selected:", layout, "cg", cgroup(), flush=True)
    ok = True
    for one, link in zip(name, url):
        print("== opening", one, flush=True)
        ok = ok and watch(page, sampler, one, link, seconds)
    for line in od.faults(page)[:20]:
        print(line[:300])
    return ok


def main():
    if len(sys.argv) not in (4, 5):
        print(__doc__.split("\n\n")[1], file=sys.stderr)
        return 2
    path, backend, layout = sys.argv[1:4]
    seconds = int(sys.argv[4]) if len(sys.argv) == 5 else 60
    server = nav.serve("app/dist")
    paths = path.split(",")
    docs, urls = od.documents(paths)
    sampler = Sampler()
    sampler.start()
    ok = False
    with tempfile.TemporaryDirectory() as profile:
        browser = nav.launch_browser(profile, extra=gpu.chrome_flags())
        try:
            page = smokecdp.Watcher(nav.DEBUG_PORT)
            page.call("Target.setAutoAttach", {"autoAttach": True, "waitForDebuggerOnStart": False, "flatten": True})
            query = "" if backend == "default" else f"?backend={backend}"
            # STUDIO_URL: a dev server outside this container (run with --network host).
            base = os.environ.get("STUDIO_URL") or f"http://127.0.0.1:{server.server_address[1]}/"
            ok = run(page, f"{base}{query}", paths, urls, layout, seconds, sampler)
        except (cdp.CdpError, OSError) as failure:
            print(f"ingest-file: the page went away: {failure}", flush=True)
        finally:
            time.sleep(1)
            print("browser exit code:", browser.poll(), "peak RSS MB by type:", sampler.peak_by_type,
                  "cgroup:", cgroup(), flush=True)
            sampler.stop = True
            browser.terminate()
            browser.wait(timeout=10)
            server.shutdown()
            docs.shutdown()
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
