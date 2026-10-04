"""The embed gate's steps that read the page: the load, the preview, the events and the storage.

A step is `step(page, ctx) -> [row]`: it drives the page, then says what it measured. `ctx` is
one run's state, shared by its steps in order: the url, the driver `hand`, the node `point` the
last `pick` found, and the fixture's own counts. The steps that drive a gesture or a load race
are in `embedgestures.py`; `embed.py` lists which steps each run takes.

The four load rows are the load smoke gate's (`smokerows.py`), renamed: the store's error, the
banner in the shadow root, the node count and the browser's error channels. The page is never
judged by its title.
"""
import json
import time

import embedpage
import smokerows as judge
import verdict
from drive import VIEWPORT

HOST_TYPES = ("graph-load", "node-select", "node-open", "node-hover", "graph-error")
OPEN_VIAS = ("dblclick", "enter", "inspector")
INSPECTOR_TITLE = "const t = el.shadowRoot.querySelector('.gs-inspector .gs-preview-title'); return t === null ? null : t.textContent;"
STORED = "Object.keys(localStorage).filter((key) => key.startsWith('graph-studio.')).sort()"
# How long a preview has to arrive once its node is selected: no debounce for the inspector.
# Caveat: 5 s is a wait for a resolve that may never come, so a host slower than that reads as one
# that answered with nothing and the row fails on the missing title rather than on a timeout. The
# way out is a `resolve` that answers in time, or a larger PREVIEW_CAP_S here.
PREVIEW_CAP_S = 5.0

# Every pause in this module, including the one inside the resolve poll: a fixed wait, never a
# wait for a condition. Caveat: a page slower than 0.4 s to answer reads as one that never will, so
# a row can fail on what it did not see yet; the way out is polling the value the step is waiting
# for, as `step_resolve` does around its own poll, and a larger SETTLE_S here.
SETTLE_S = 0.4


def renamed(row, name):
    return dict(row, row=name)


def no_point(name, expectation):
    return verdict.row(name, expectation, "no node found on the canvas", False,
                       "the canvas showed no node a pointer could reach")


def studio_of(health):
    return None if health is None else health.get("studio")


def step_load(page, ctx):
    """Load the page, wait for it to answer, and judge the load as the smoke gate would."""
    page.set_viewport(VIEWPORT[0], VIEWPORT[1], 1)
    # Before the first navigation: the harness's own `document` listener, for `step_composed`.
    embedpage.install_harness(page)
    page.navigate("about:blank")
    page.navigate(ctx["url"])
    at = studio_of(embedpage.wait_loaded(page))
    if at is not None and at["nodes"] > 0:
        embedpage.settle_drawing(page)
    health = page.evaluate(embedpage.HEALTH)
    page.screenshot(ctx["out"] / f"studio-embed-{ctx['run']}.png")
    at = studio_of(health)
    return [renamed(judge.row_no_store_error(at), "embed-no-store-error"),
            renamed(judge.row_no_overlay(at), "embed-no-overlay"),
            renamed(judge.row_drew_nodes(at), "embed-drew-nodes"),
            row_host_load(health, ctx["fixture"])]


def row_host_load(health, fixture):
    nodes, edges = fixture["nodes"], fixture["edges"]
    expectation = (f"the page's loadGraph resolves with the fixture's {nodes} nodes, and the first graph-load "
                   f"`document` hears comes after the call and says {nodes} nodes, {edges} edges, with notes")
    shown = None if health is None else health.get("page")
    if shown is None:
        return verdict.row("embed-host-load", expectation, "no window.__embed on the page", False)
    loads = [(at, heard["detail"]) for at, heard in enumerate(shown["heard"]) if heard["type"] == "graph-load"]
    first = loads[0] if loads else None
    told = "none" if first is None else f"#{first[0]} (the call was at #{shown['loadCalledAt']}): {json.dumps(first[1])}"
    passed = (shown["state"] == f"loaded {nodes}" and first is not None and first[0] >= shown["loadCalledAt"] >= 0
              and first[1]["nodes"] == nodes and first[1]["edges"] == edges and isinstance(first[1]["notes"], list))
    return verdict.row("embed-host-load", expectation,
                       f"state `{shown['state']}`; first graph-load {judge.short(told)}", passed)


# A columnar document the motor must refuse: both node rows carry the id 0, and the refusal is that
# repeated id alone — `index_columns` refuses a duplicate rather than merging the two rows, so nothing
# else in the document is at fault (`docs/contract/ingest-columns.md`, the refusal table). The cells
# are one `u32` column per field at `2 * r + column` for the two rows, in the contract's order: id,
# kind, database, source, label, group, icon, has_note. `absent` is `u32::MAX`, the mark of a column
# the rows leave out.
COLUMNS_REFUSED = """
const start = window.__embed.heard.length;
const absent = 0xffffffff; const ids = [0, 0]; const kind = [1, 1]; const source = [2, 2];
const nodeCells = new Uint32Array(16);
for (let r = 0; r < 2; r += 1) {
  nodeCells[r] = ids[r]; nodeCells[2 + r] = kind[r]; nodeCells[4 + r] = absent; nodeCells[6 + r] = source[r];
  nodeCells[8 + r] = ids[r]; nodeCells[10 + r] = absent; nodeCells[12 + r] = absent; nodeCells[14 + r] = 0;
}
const rows = { strings: ["a", "record", "file"], nodeCells, edgeCells: new Uint32Array(0), weights: new Float64Array([0.5, 0.5]), versions: new Float64Array([0, 0]), strengths: new Float64Array(0) };
let name = 'resolved';
try { await el.loadColumns(rows); } catch (error) { name = error instanceof Error ? error.name : String(error); }
await new Promise((done) => setTimeout(done, 300));
return { name, errors: window.__embed.heard.slice(start).filter((h) => h.type === 'graph-error').map((h) => h.detail), hint: el.studio && el.studio.store.get().error ? el.studio.store.get().error.hint : null };
"""


def step_columns_refused(page, ctx):
    """The refused columnar load: the class name, the ABI code behind it, and the studio's own hint."""
    name = "embed-columns-refused"
    expectation = ("loadColumns on a two-row columnar document whose node id is repeated rejects as "
                   "ColumnsRefusedError, the one graph-error `document` hears says `code 23 (ColumnsInvalid)`, "
                   "and the studio's own error carries a hint that is not the default one")
    refused = embedpage.on_element(page, COLUMNS_REFUSED)
    errors, hint = refused["errors"], refused["hint"]
    passed = (refused["name"] == "ColumnsRefusedError" and len(errors) == 1
              and errors[0]["error"] == "code 23 (ColumnsInvalid)"
              and isinstance(hint, str) and "Unexpected studio error" not in hint)
    measured = judge.short(f"rejected as {json.dumps(refused['name'])}; graph-error {json.dumps(errors)}; "
                           f"hint {json.dumps(hint)}", 400)
    return [verdict.row(name, expectation, measured, passed)]


def step_pick(page, ctx):
    """The node nearest the centre that a pointer can reach now; re-run after a panel opens."""
    ctx["point"] = page.evaluate(embedpage.NODE_POINT)
    return []


def step_select(page, ctx):
    """A pointer moved onto the node, then a click on it: node-hover, node-select, both previews."""
    point = ctx["point"]
    if point is None:
        return []
    page.call("Input.dispatchMouseEvent", {"type": "mouseMoved", "x": point["x"], "y": point["y"],
                                           "button": "none", "buttons": 0})
    time.sleep(SETTLE_S)
    ctx["hand"].click((point["x"], point["y"]))
    time.sleep(SETTLE_S)
    selected = embedpage.on_element(page, "return el.selectedIds;")
    ctx["selected_by"] = "a click"
    if selected != [point["id"]]:
        embedpage.on_element(page, f"return el.selectNodes([{json.dumps(point['id'])}]);")
        ctx["selected_by"] = f"selectNodes, the click having selected {json.dumps(selected)}"
    return []


def step_resolve(page, ctx):
    name = "embed-resolve-upgrade"
    expectation = ("a resolve the page set before the element was defined answers for the selected node: "
                   "the inspector reads `Host record <id>`, and resolve was asked about the id")
    point = ctx["point"]
    if point is None:
        return [no_point(name, expectation)]
    want = f"Host record {point['id']}"
    deadline = time.monotonic() + PREVIEW_CAP_S
    while True:
        title = embedpage.on_element(page, INSPECTOR_TITLE)
        asked = page.evaluate("window.__embed.asked")
        if (title == want and point["id"] in asked) or time.monotonic() > deadline:
            break
        time.sleep(SETTLE_S)
    measured = (f"inspector title {json.dumps(title)}; resolve asked about {judge.short(json.dumps(asked), 120)} "
                f"(selected by {ctx.get('selected_by', '?')})")
    return [verdict.row(name, expectation, measured, title == want and point["id"] in asked)]


def step_channels(page, ctx):
    """The browser's error channels over the whole run so far, page and motor worker alike."""
    page.call("Runtime.evaluate", {"expression": "1"})  # drain what is still in the socket
    page.watch_workers()
    return [renamed(judge.row_no_exception(page), "embed-no-exception"),
            renamed(judge.row_no_console_error(page), "embed-no-console-error")]


def plain_ids(heard):
    """True when the detail carries the host's string ids, and only plain data, for its type."""
    detail = heard["detail"]
    checks = {
        "graph-load": lambda: (isinstance(detail["nodes"], int) and isinstance(detail["edges"], int)
                               and all(isinstance(note, str) for note in detail["notes"])),
        "node-select": lambda: all(isinstance(each, str) for each in detail["ids"]),
        "node-open": lambda: isinstance(detail["id"], str) and detail["via"] in OPEN_VIAS,
        "node-hover": lambda: detail["id"] is None or isinstance(detail["id"], str),
        "graph-error": lambda: isinstance(detail["error"], str) and isinstance(detail["message"], str),
    }
    try:
        return checks[heard["type"]]()
    except (KeyError, TypeError):
        return False


def step_composed(page, ctx):
    """The browser's own `Event` properties, read by the harness's listener, not by the page.

    `embedpage.HARNESS` is installed over CDP before the page's scripts, so `bubbles`, `composed`
    and `frozen` are read off the event the browser dispatched and not off anything the page under
    test recorded about it; a page that wrote `composed: true` for an event it never composed fails
    here.
    """
    name = "embed-composed"
    expectation = ("a `document` listener outside the page's own shadow root hears all five events, each "
                   "bubbling and composed, with a frozen detail of string ids")
    heard = page.evaluate("window.__harness.heard")
    types = sorted({each["type"] for each in heard})
    missing = [kind for kind in HOST_TYPES if kind not in types]
    malformed = sorted({each["type"] for each in heard
                        if not (each["bubbles"] and each["composed"] and each["frozen"] and plain_ids(each))})
    measured = (f"{len(heard)} heard, types {', '.join(types) or 'none'}; missing {', '.join(missing) or 'none'}; "
                f"malformed {', '.join(malformed) or 'none'}")
    return [verdict.row(name, expectation, measured, not missing and not malformed)]


def step_storage(page, ctx):
    name = "embed-storage"
    expectation = ("no `graph-studio.` key in localStorage after a load and after a reload, and the reload's one "
                   "graph-load is the host's own, heard after its loadGraph call")
    before = page.evaluate(STORED)
    page.navigate("about:blank")
    page.navigate(ctx["url"])
    embedpage.wait_loaded(page)
    time.sleep(SETTLE_S)
    shown = page.evaluate("({ heard: window.__embed.heard, at: window.__embed.loadCalledAt })")
    after = page.evaluate(STORED)
    loads = [(at, heard["detail"]["nodes"]) for at, heard in enumerate(shown["heard"]) if heard["type"] == "graph-load"]
    measured = (f"keys after the load {json.dumps(before)}, after the reload {json.dumps(after)}; the reload's graph-loads "
                f"{json.dumps(loads)} (index, nodes), the call at #{shown['at']}")
    passed = (not before and not after and len(loads) == 1 and loads[0][0] >= shown["at"] >= 0
              and loads[0][1] == ctx["fixture"]["nodes"])
    return [verdict.row(name, expectation, judge.short(measured, 400), passed)]
