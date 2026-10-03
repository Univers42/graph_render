"""The rows of the service-image gate: the container, its HTTP surface, and the bundle in a browser.

The load rows are the smoke gate's own (`smokerows.py`), renamed per page, so "the studio came up"
means the same thing here as it does over app/dist. What this module adds is the service: who the
process runs as, the auth answer with and without the key, the headers on the wasm, and which of
the two wasm builds the motor worker asked for.
"""
import urllib.error
import urllib.request

import smokerows
import verdict

WASM = ("graph_wasm.wasm", "graph_wasm_threads.wasm")
# What every versioned embed file carries (docs/contract/service-api.md "Headers" and C10).
EMBED_HEADERS = {"cross-origin-embedder-policy": "require-corp", "cross-origin-resource-policy": "cross-origin",
                 "x-content-type-options": "nosniff", "cache-control": "public, max-age=31536000, immutable"}
ASSETS = {name: {"content-type": "application/wasm", **EMBED_HEADERS} for name in WASM}
ASSETS["graph-studio.js"] = {"content-type": "text/javascript", **EMBED_HEADERS}


def fetch(url, key=None):
    """Status and headers of a GET; a refusal is an answer here, not an exception."""
    headers = {} if key is None else {"Authorization": f"Bearer {key}"}
    try:
        reply = urllib.request.urlopen(urllib.request.Request(url, headers=headers), timeout=10)
    except urllib.error.HTTPError as refused:
        reply = refused
    with reply:
        reply.read()
        return reply.status, reply.headers


def container_rows(uid, health, leaks):
    """`leaks` is scripts/service-image.sh's scan of the image: one offending path per entry."""
    found = "; ".join(leaks[:5]) + (f"; and {len(leaks) - 5} more" if len(leaks) > 5 else "")
    return [
        verdict.row("svc-non-root", "`docker exec id -u` is not 0", f"uid {uid}", uid not in ("", "0")),
        verdict.row("svc-healthy", "the image's HEALTHCHECK reports healthy", health, health == "healthy"),
        verdict.row("svc-no-leak", "no key file, .env, .git or build-host path in the image",
                    found or "none found", not leaks),
    ]


def status_row(name, expectation, answer, wanted):
    status, _ = answer
    return verdict.row(name, expectation, f"HTTP {status}", status == wanted)


def auth_rows(service, key):
    """The key is never part of a row: only the status it got."""
    return [
        status_row("svc-healthz", "GET /healthz is 200", fetch(f"{service}/healthz"), 200),
        status_row("svc-meta-no-key", "GET /v1/meta without a key is 401", fetch(f"{service}/v1/meta"), 401),
        status_row("svc-meta-key", "GET /v1/meta with the test key is 200", fetch(f"{service}/v1/meta", key), 200),
    ]


def media_type(value):
    return (value or "").split(";", 1)[0].strip().lower()


def asset_row(service, version, name):
    """One versioned file: 200 and every header in ASSETS, the content type up to its parameters."""
    wanted = ASSETS[name]
    status, headers = fetch(f"{service}/embed/{version}/{name}")
    seen = {header: headers.get(header) for header in wanted}
    seen["content-type"] = media_type(seen["content-type"])
    measured = f"HTTP {status}, " + ", ".join(f"{h}: {v}" for h, v in seen.items())
    passed = status == 200 and seen == wanted
    expectation = "200, " + ", ".join(f"{h}: {v}" for h, v in wanted.items())
    return verdict.row(f"svc-embed-{name}", expectation, measured, passed)


def unversioned_row(service):
    """Outside /embed/<version>/ nothing is cached for good (C10): the server's VERSION file included."""
    answers = {path: fetch(f"{service}{path}") for path in ("/embed/graph-studio.js", "/embed/VERSION")}
    measured = "; ".join(f"{p} HTTP {s}, cache-control {h.get('cache-control')}" for p, (s, h) in answers.items())
    passed = all(s == 404 or "no-cache" in (h.get("cache-control") or "") for s, h in answers.values())
    return verdict.row("svc-embed-unversioned", "404 or no-cache outside /embed/<version>/", measured, passed)


def http_rows(service, version, key):
    return [*auth_rows(service, key), *(asset_row(service, version, name) for name in ASSETS),
            unversioned_row(service)]


def renamed(row, prefix):
    return {**row, "row": row["row"].replace("smoke-", f"{prefix}-", 1)}


def load_rows(page, at, prefix):
    """The smoke gate's rows that say the studio came up, under this page's name."""
    rows = [smokerows.row_no_exception(page), smokerows.row_no_console_error(page),
            smokerows.row_no_store_error(at), smokerows.row_no_overlay(at), smokerows.row_drew_nodes(at)]
    return [renamed(row, prefix) for row in rows]


def isolation_row(page, at, prefix, isolated):
    """`crossOriginIsolated` on the page and on every motor worker, which must all equal `isolated`."""
    expectation = f"crossOriginIsolated is {smokerows.flag(isolated)} on the page and every worker"
    if at is None:
        return verdict.row(f"{prefix}-isolation", expectation, "no studio on the page", False)
    workers = [smokerows.worker_isolated(page, s) for s in smokerows.worker_sessions(page)]
    measured = f"page {smokerows.flag(at['isolated'])}, workers " + (
        ", ".join(smokerows.flag(w) for w in workers) or "none attached")
    passed = bool(workers) and at["isolated"] is isolated and all(w is isolated for w in workers)
    return verdict.row(f"{prefix}-isolation", expectation, measured, passed)


def module_row(requested, cores, prefix, isolated):
    """Which wasm build the worker fetched through the host, read from the host's own log.

    Caveat: the motor asks for threads only with two cores or more (`motor/threads.ts`), so on a
    one-core host the isolated page loads the serial build and this row fails; `cores` says so.
    """
    wanted = WASM[1] if isolated else WASM[0]
    fetched = sorted({path.rsplit("/", 1)[-1] for path in requested if path.endswith(".wasm")})
    measured = f"fetched {', '.join(fetched) or 'no wasm'} ({cores} cores)"
    return verdict.row(f"{prefix}-wasm-build", f"the worker fetched {wanted} and no other wasm",
                       measured, fetched == [wanted])


def direct_row(page, at):
    """The bundle from the service's own origin: documented to fail on the Worker's origin check."""
    expectation = "the studio does not come up, and the failure names the Worker"
    named = (smokerows.events(page, "Runtime.exceptionThrown")
             + smokerows.faults(page, "Runtime.consoleAPICalled", ("type",), ("error",))
             + smokerows.faults(page, "Log.entryAdded", ("entry", "level"), ("error",)))
    nodes = 0 if at is None else at["nodes"]
    measured = f"{nodes} nodes drawn; " + ("; ".join(named) or "no error reported")
    passed = nodes == 0 and any("Worker" in text for text in named)
    return verdict.row("direct-refused", expectation, smokerows.short(measured), passed)
