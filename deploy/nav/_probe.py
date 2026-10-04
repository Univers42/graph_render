#!/usr/bin/env python3
"""TEMPORARY probe: one batch, timed, with the worker's own words. Deleted before return."""
import json
import sys
import tempfile
import time

sys.path.insert(0, "/w/deploy/nav")
import delta
import nav
import smokecdp
import smokerows
from drive import Studio
from liverows import HOST


def main():
    args = nav.parse_args()
    server = nav.serve(args.dist)
    url = f"http://127.0.0.1:{server.server_address[1]}/"
    with tempfile.TemporaryDirectory() as profile:
        browser = nav.launch_browser(profile)
        try:
            page = smokecdp.Watcher(nav.DEBUG_PORT)
            page.start_watching()
            studio = Studio(page, url, wait_for_settle=False)
            studio.open()
            page.watch_workers()
            print("opened", flush=True)
            studio.page.evaluate(f"{HOST}.studio.dispatch('forces.animate', {{ on: true }})")
            print("animate", flush=True)
            studio.page.evaluate(f"""
            (() => {{
              const el = {HOST};
              window.__frames = 0;
              const set = el.view.setFrame.bind(el.view);
              el.view.setFrame = (...args) => {{ window.__frames += 1; return set(...args); }};
            }})()
            """)
            print("patched", flush=True)
            for call in range(1):
                batch = delta.batch_json(f"probe-{call}", 10)
                began = time.monotonic()
                try:
                    out = studio.page.evaluate(f"""
                (async () => {{
                  const el = {HOST};
                  const t0 = performance.now();
                  try {{ return {{ ok: true, answer: await el.applyDeltas({batch}),
                                   ms: performance.now() - t0 }}; }}
                  catch (failure) {{ return {{ ok: false, name: failure.name,
                                   detail: String(failure.message), ms: performance.now() - t0 }}; }}
                }})()
                """)
                    print(f"call {call} took {time.monotonic() - began:.2f}s -> {json.dumps(out)}", flush=True)
                except Exception as failure:  # noqa: BLE001 — a probe prints what it saw
                    print(f"call {call} hung after {time.monotonic() - began:.2f}s: {failure}", flush=True)
                for _ in range(8):
                    time.sleep(0.25)
                    studio.page.watch_workers()
                    counts = studio.page.evaluate(
                        f"[{HOST}.view.frame().nodeCount, {HOST}.studio.store.get().meta.nodeCount]")
                    print("counts", counts, "setFrame calls",
                          studio.page.evaluate("window.__frames"), flush=True)
                print("events", json.dumps([e["method"] for e in page.events])[:600], flush=True)
                print("console", json.dumps(smokerows.events(page, "Runtime.consoleAPICalled"))[:1500], flush=True)
                print("exc", json.dumps(smokerows.events(page, "Runtime.exceptionThrown"))[:1500], flush=True)
                print("log", json.dumps(smokerows.faults(page, "Log.entryAdded", "entry", ["error"]))[:1500], flush=True)
            return 0
        finally:
            browser.terminate()
            browser.wait(timeout=10)
            server.shutdown()


if __name__ == "__main__":
    sys.exit(main())