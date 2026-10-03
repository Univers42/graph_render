"""Service-image gate, the probe half: the service's HTTP surface and the embed bundle in Chromium.

Usage: service.py --version V --uid UID --health STATE --leaks FILE --out DIR [--break] [--commit ID]
Exit:  0 every row PASS · 1 a row FAIL or NOT-RUN · 2 the harness could not run

scripts/service-image.sh runs this in gm-chromium on the service container's own network, so the
service is 127.0.0.1:8080 here and the host pages are other ports of the same loopback: other
origins, and every one a secure context. The test key arrives in GRAPH_TEST_KEY and goes nowhere
but a request header. The host pages are in `serviceproxy.py`; the rows in `servicerows.py`.

`--break` is the negative control: a pass-through in front of the service drops COOP, COEP and
CORP from every reply, and every row then reads the service through it.
"""
import argparse
import json
import os
import sys
import tempfile
import time
from pathlib import Path

import nav  # first: it puts the perf gate's CDP client on the path
import cdp
import smoke
import smokecdp
import smokerows
import serviceproxy as proxy
import servicerows as judge

SERVICE = "http://127.0.0.1:8080"
# Caveat: a fixed cap. The direct page fails at mount, well inside it on a quiet host; on a host
# so loaded that it takes longer, the row reads "no error reported" and fails, never passes.
DIRECT_CAP_S = 20.0
# The direct page's module may never load (no CORS), and then the element is never defined.
DIRECT_PROBE = f"(customElements.get('graph-studio') === undefined ? null : {smokerows.PROBE.strip()})"


def browse(url, shot, read):
    """`read` over a fresh browser on `url`: a new profile, so no page meets the last one's cache."""
    with tempfile.TemporaryDirectory() as profile:
        browser = nav.launch_browser(profile)
        try:
            page = smokecdp.Watcher(nav.DEBUG_PORT)
            page.start_watching()
            return read(page, url, shot)
        finally:
            browser.terminate()
            browser.wait(timeout=10)


def embedded(requested, prefix, isolated):
    """The rows of one proxied page, as a `read` for `browse`."""
    def read(page, url, shot):
        requested.clear()
        at = smoke.settle(page, url)
        page.call("Runtime.evaluate", {"expression": "1"})  # drain what is still in the socket
        page.watch_workers()
        page.screenshot(shot)
        cores = page.evaluate("navigator.hardwareConcurrency")
        return [*judge.load_rows(page, at, prefix), judge.isolation_row(page, at, prefix, isolated),
                judge.module_row(requested, cores, prefix, isolated)]
    return read


def faulted(page):
    return any(e["method"] == "Runtime.exceptionThrown"
               or (e["method"] == "Log.entryAdded" and e["params"].get("entry", {}).get("level") == "error")
               for e in page.events)


def direct(page, url, shot):
    page.navigate("about:blank")
    page.navigate(url)
    deadline = time.monotonic() + DIRECT_CAP_S
    while time.monotonic() < deadline and not faulted(page):
        page.call("Runtime.evaluate", {"expression": "1"})
        time.sleep(0.2)
    at = page.evaluate(DIRECT_PROBE)
    page.screenshot(shot)
    return [judge.direct_row(page, at)]


def browser_rows(host, requested, out):
    base = proxy.origin(host)
    rows = []
    for prefix, isolated in (("isolated", True), ("plain", False)):
        rows += browse(f"{base}/{prefix}/", out / f"{prefix}.png", embedded(requested, prefix, isolated))
    return rows + browse(f"{base}/direct/", out / "direct.png", direct)


def measure(args, key):
    breaker = proxy.start(SERVICE, strip=True)[0] if args.broken else None
    service = SERVICE if breaker is None else proxy.origin(breaker)
    host, requested = proxy.start(service, proxy.host_pages(args.version, service))
    try:
        leaks = [line for line in args.leaks.read_text().splitlines() if line]
        rows = [*judge.container_rows(args.uid, args.health, leaks), *judge.http_rows(service, args.version, key)]
        rows += browser_rows(host, requested, args.out)
        return {"label": args.out.name, "commit": args.commit, "version": args.version,
                "break": args.broken, "rows": rows}
    finally:
        host.shutdown()
        if breaker is not None:
            breaker.shutdown()


def table(report):
    head = [f"# service-image — {report['label']}", "",
            f"commit `{report['commit']}` · image graph-motor:{report['version']} · break {report['break']}"
            " · screenshots `isolated.png`, `plain.png`, `direct.png`", "",
            "| row | expectation | measured | verdict |", "|---|---|---|---|"]
    body = [f"| `{r['row']}` | {r['expectation']} | {r['measured']} | {r['verdict']} |" for r in report["rows"]]
    notes = [f"`{r['row']}` not run: {r['why']}" for r in report["rows"] if r["verdict"] == "NOT-RUN"]
    return "\n".join([*head, *body, "", *(notes or ["no row was left unrun"]), ""])


def parse_args():
    parser = argparse.ArgumentParser(description=__doc__.split("\n", 1)[0])
    parser.add_argument("--version", required=True)
    parser.add_argument("--uid", required=True)
    parser.add_argument("--health", required=True)
    parser.add_argument("--leaks", type=Path, required=True, help="the image scan, one offending path per line")
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--commit", default="unknown")
    parser.add_argument("--break", action="store_true", dest="broken",
                        help="the negative control: read the service through a pass-through that "
                             "drops COOP, COEP and CORP")
    return parser.parse_args()


def main():
    args = parse_args()
    key = os.environ.get("GRAPH_TEST_KEY", "")
    if not key:
        print("service-image: GRAPH_TEST_KEY is unset", file=sys.stderr)
        return 2
    args.out.mkdir(parents=True, exist_ok=True)
    try:
        report = measure(args, key)
    except (cdp.CdpError, OSError) as failure:
        print(f"service-image: could not run: {failure}", file=sys.stderr)
        return 2
    (args.out / "report.json").write_text(json.dumps(report, indent=1) + "\n")
    printed = table(report)
    (args.out / "table.md").write_text(printed)
    print(printed)
    return 0 if all(r["verdict"] == "PASS" for r in report["rows"]) else 1


if __name__ == "__main__":
    sys.exit(main())
