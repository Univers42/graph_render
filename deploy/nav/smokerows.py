"""The rows of the load smoke gate: what the page threw, logged and drew between load and settle.

Nothing here reads a graph property or drives input. Every row is a claim about whether the studio
came up at all — the 2026-10-01 failure was a build that served a wasm older than the SDK, the
studio died on load, and every browser gate still passed because each one only checked its own
rows. So these rows read the four places a load failure shows itself: the browser's error
channels, the studio's own store, the overlay a user would read — and the node count, which is
the difference between a studio and a blank canvas.

Every position of the failure is read from the page as a user would meet it: the store through the
element's own `studio`, the banner out of the shadow root, the node count from the view.
"""
import verdict

# How much of a failure the row prints. The missing-export list alone is 600 characters.
DETAIL_CHARS = 200

# One probe, read once after the settle: the store's error, the banner's text and the node count.
PROBE = """
(() => {
  const host = document.querySelector('graph-studio');
  if (host === null || host.studio === null) return null;
  const at = host.studio.store.get();
  const alert = host.shadowRoot === null ? null : host.shadowRoot.querySelector('.gs-alert');
  const view = host.view;
  return {
    error: at.error, meta: at.meta, busy: at.busy.length,
    nodes: view === null ? 0 : view.stats().nodes,
    banner: alert === null ? null : alert.textContent,
    isolated: crossOriginIsolated,
  };
})()
"""


def short(value, limit=DETAIL_CHARS):
    """A failure, on one line and no longer than the table."""
    text = value if isinstance(value, str) else str(value)
    text = " ".join(text.split())
    return text if len(text) <= limit else text[:limit] + "…"


def flag(value):
    """A page-side boolean as JavaScript spells it: `true`, `false`, `null` for no value."""
    if value is True:
        return "true"
    if value is False:
        return "false"
    return "null" if value is None else str(value)


def described(event):
    """The one line that names a thrown or logged failure."""
    if event["method"] == "Runtime.exceptionThrown":
        detail = event["params"].get("exceptionDetails", {})
        # `text` is the bare "Uncaught"; the exception's own description carries the message and
        # the top of the stack. Three lines is enough to name it and short enough for the table.
        thrown = detail.get("exception", {}).get("description") or detail.get("text") or "?"
        return short("\n".join(thrown.split("\n")[:3]))
    if event["method"] == "Runtime.consoleAPICalled":
        call = event["params"]
        text = " ".join(str(a.get("value", a.get("description", a.get("type")))) for a in call["args"])
        return short(f"{call.get('type')}: {text}")
    entry = event["params"].get("entry", {})
    return short(f"{entry.get('level')} {entry.get('source')}: {entry.get('text')} {entry.get('url') or ''}")


def events(page, method):
    """Every collected event of `method`, described, page and motor worker alike.

    The domains name their payload differently, so there is no one field to filter on: the
    console rows filter through `faults`, the exception row takes the whole method.
    """
    return [described(e) for e in page.events if e["method"] == method]


def faults(page, method, path, wanted):
    """The events of `method` whose `params` value at `path` is one of `wanted`, described.

    `consoleAPICalled` has `type` at the top of the payload and `Log.entryAdded` has `entry.level`,
    so the field is named by path.
    """
    return [described(e) for e in page.events
            if e["method"] == method and dig(e["params"], path) in wanted]


def dig(params, path):
    value = params
    for key in path:
        value = {} if not isinstance(value, dict) else value.get(key)
    return value


def worker_sessions(page):
    """Every attached motor worker's session, after one more watch for a late attach."""
    page.watch_workers()
    return [e["params"]["sessionId"] for e in page.events
            if e["method"] == "Target.attachedToTarget"
            and e["params"].get("targetInfo", {}).get("type") == "worker"]


def worker_isolated(page, session):
    """`crossOriginIsolated` read over the worker's own session: the page's is not its."""
    reply = page.session_call(session, "Runtime.evaluate",
                              {"expression": "crossOriginIsolated", "returnByValue": True})
    return reply.get("result", {}).get("value")


def row_no_exception(page):
    expectation = "no uncaught exception from load until the drawing settles, page or motor worker"
    thrown = events(page, "Runtime.exceptionThrown")
    measured = "none" if not thrown else f"{len(thrown)}: " + "; ".join(thrown)
    return verdict.row("smoke-no-exception", expectation, measured, not thrown)


def row_no_console_error(page):
    """Both halves of the browser console: `console.error` calls and the browser's own log entries.

    `Log.entryAdded` is not redundant with `consoleAPICalled` here: the loader's refusal is a
    `console.error` inside the motor worker, and a wasm served with the wrong MIME type or a
    refused request is a `Log` entry with no call behind it.

    Ponytail: no allowlist. A `Log` entry at level `error` is a failure the browser reported, and
    the decision whether one is expected belongs to a row that expects one — not to a filter here.
    """
    expectation = "no console error and no Log error from load until the drawing settles"
    called = faults(page, "Runtime.consoleAPICalled", ("type",), ("error", "assert"))
    logged = faults(page, "Log.entryAdded", ("entry", "level"), ("error",))
    measured = "none" if not (called or logged) else "; ".join([*called, *logged])
    return verdict.row("smoke-no-console-error", expectation, measured, not (called or logged))


def row_no_store_error(at):
    expectation = "studio.store.get().error is null once the studio has come up"
    if at is None:
        return verdict.row("smoke-no-store-error", expectation, "no studio on the page", False, "the element never defined")
    error = at["error"]
    measured = "null" if error is None else f"{error['title']}: {short(error['detail'])}"
    return verdict.row("smoke-no-store-error", expectation, measured, error is None)


def row_no_overlay(at):
    expectation = "no failure banner in the studio's shadow root"
    if at is None:
        return verdict.row("smoke-no-overlay", expectation, "no studio on the page", False, "the element never defined")
    banner = at["banner"]
    measured = "none" if banner is None else short(banner)
    return verdict.row("smoke-no-overlay", expectation, measured, banner is None)


def row_drew_nodes(at):
    expectation = "the view drew at least one node"
    if at is None:
        return verdict.row("smoke-drew-nodes", expectation, "no studio on the page", False, "the element never defined")
    nodes = at["nodes"]
    return verdict.row("smoke-drew-nodes", expectation, f"{nodes} nodes drawn", nodes > 0)


def row_cross_origin_isolated(page, at):
    """Both halves read where they run: the page's own flag, each worker's over its session.

    The negative control is `smoke.py --break-coi`, which serves without the COOP/COEP headers:
    neither half reports true and the row fails — with no `why`, because a page served unisolated
    is a measured failure, not a row the harness could not read.
    """
    expectation = "crossOriginIsolated is true for the page and for every attached motor worker"
    if at is None:
        return verdict.row("smoke-cross-origin-isolated", expectation,
                           "no studio on the page", False, "the element never defined")
    sessions = worker_sessions(page)
    if not sessions:
        return verdict.row("smoke-cross-origin-isolated", expectation,
                           "no motor worker attached", False)
    workers = [worker_isolated(page, session) for session in sessions]
    measured = (f"page {flag(at['isolated'])}, motor worker "
                + ", ".join(flag(worker) for worker in workers))
    passed = at["isolated"] is True and all(worker is True for worker in workers)
    return verdict.row("smoke-cross-origin-isolated", expectation, measured, passed)


def run_rows(page, at):
    return [row_no_exception(page), row_no_console_error(page), row_no_store_error(at),
            row_no_overlay(at), row_drew_nodes(at), row_cross_origin_isolated(page, at)]
