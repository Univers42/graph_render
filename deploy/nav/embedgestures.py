"""The embed gate's steps that act: the three ways to open a node, and two loads raced.

Every gesture is real input over CDP (`drive.Studio`), so nothing passes that a user's hand could
not do. The page API is called only where the row is about the page API: `selectNodes` sets up a
selection a gesture then acts on, and `loadGraph` is what the overlap and refusal rows measure.
"""
import json
import time

import embedpage
import verdict
from embedrows import no_point

# Verdict 11: how many Tab presses the inspector's Open button may be behind. The dock, the search
# box and every panel's controls come first; past this the button is called unreachable.
TAB_CAP = 200

WHERE_FOCUS = """
const inside = document.querySelector('#frame').shadowRoot.activeElement === el;
const deep = el.shadowRoot.activeElement;
if (!inside) return 'outside';
return deep === null ? 'element' : deep.matches('.gs-open') ? 'open' : (deep.className || deep.tagName);
"""

OPEN_CENTRE = """
const button = el.shadowRoot.querySelector('.gs-inspector .gs-open');
if (button === null) return null;
const box = button.getBoundingClientRect();
const centre = [box.left + box.width / 2, box.top + box.height / 2];
return el.shadowRoot.elementFromPoint(centre[0], centre[1]) === button ? centre : null;
"""


def opened(page, start, via):
    """The node-open details `document` heard since `start` that came by `via`."""
    return [heard["detail"] for heard in embedpage.heard_since(page, start)
            if heard["type"] == "node-open" and heard["detail"].get("via") == via]


def select(page, ids):
    return embedpage.on_element(page, f"return el.selectNodes({json.dumps(ids)});")


def step_dblclick(page, ctx):
    name = "embed-dblclick"
    expectation = "a real double click on a node: exactly one node-open {id, via: dblclick}, and #opened shows the id"
    point = ctx["point"]
    if point is None:
        return [no_point(name, expectation)]
    start = page.evaluate(embedpage.HEARD_COUNT)
    ctx["hand"].double_click((point["x"], point["y"]))
    time.sleep(0.4)
    opens = opened(page, start, "dblclick")
    shown = page.evaluate("document.getElementById('opened').textContent")
    passed = opens == [{"id": point["id"], "via": "dblclick"}] and shown == point["id"]
    return [verdict.row(name, expectation, f"node-open {json.dumps(opens)}; #opened reads {json.dumps(shown)}", passed)]


# The four Enter cases of verdict 11: what is selected, what has the focus, and the opens expected.
ENTER_CASES = (
    ("one selected, the element focused", "one", "el.focus();", "element", 1),
    ("two selected, the element focused", "two", "el.focus();", "element", 0),
    ("one selected, the search box focused", "one",
     "const box = el.shadowRoot.querySelector('.gs-search .gs-input'); if (box !== null) box.focus();", "gs-input", 0),
    ("one selected, the element blurred", "one", "el.blur();", "outside", 0),
)


def enter_case(page, ctx, case):
    """One Enter press under `case`; returns (focus as found, node-open count, as expected)."""
    _, picked, focus, where_wanted, wanted = case
    point = ctx["point"]
    select(page, [point["id"]] if picked == "one" else [point["id"], point["other"]])
    where = embedpage.on_element(page, f"{focus} {WHERE_FOCUS}")
    start = page.evaluate(embedpage.HEARD_COUNT)
    ctx["hand"].key("Enter")
    time.sleep(0.3)
    opens = opened(page, start, "enter")
    count_right = len(opens) == wanted and all(each["id"] == point["id"] for each in opens)
    return where, len(opens), count_right and where_wanted in where


def step_enter(page, ctx):
    name = "embed-enter"
    expectation = ("Enter opens the one selected node (via enter) only while the element itself has the focus: "
                   "1 open, then 0 with two selected, 0 from the search box, 0 once blurred")
    point = ctx["point"]
    if point is None or point["other"] is None:
        return [no_point(name, expectation)]
    results = [(case[0], *enter_case(page, ctx, case)) for case in ENTER_CASES]
    select(page, [point["id"]])
    measured = "; ".join(f"{label}: focus on {where}, {count} open" for label, where, count, _ in results)
    return [verdict.row(name, expectation, measured, all(right for *_, right in results))]


def open_centre(page, ctx):
    """The page point at the centre of the inspector's Open button, once one node is selected."""
    select(page, [ctx["point"]["id"]])
    deadline = time.monotonic() + 3.0
    while time.monotonic() < deadline:
        centre = embedpage.on_element(page, OPEN_CENTRE)
        if centre is not None:
            return centre
        time.sleep(0.2)
    return None


def tabs_to_open(page, hand):
    """How many Tab presses from the element reach the Open button; None if focus left first."""
    where = embedpage.on_element(page, f"el.focus(); {WHERE_FOCUS}")
    for press in range(1, TAB_CAP + 1):
        hand.key("Tab")
        where = embedpage.on_element(page, WHERE_FOCUS)
        if where in ("open", "outside"):
            return press if where == "open" else None
    return None


def step_open(page, ctx):
    name = "embed-open"
    expectation = (f"a real click on the inspector's Open button: exactly one node-open {{id, via: inspector}}; "
                   f"and the button is reached by Tab from the element within {TAB_CAP} presses")
    if ctx["point"] is None:
        return [no_point(name, expectation)]
    centre = open_centre(page, ctx)
    if centre is None:
        return [verdict.row(name, expectation, "no Open button a pointer can reach", False)]
    start = page.evaluate(embedpage.HEARD_COUNT)
    ctx["hand"].click(tuple(centre))
    time.sleep(0.4)
    opens = opened(page, start, "inspector")
    presses = tabs_to_open(page, ctx["hand"])
    clicked = opens == [{"id": ctx["point"]["id"], "via": "inspector"}]
    tabbed = "focus left the element first" if presses is None else f"reached after {presses} Tab presses"
    return [verdict.row(name, expectation, f"node-open {json.dumps(opens)}; Open button {tabbed}",
                        clicked and presses is not None)]


OVERLAP = """
const chain = (n, p) => ({ version: 1, nodes: Array.from({ length: n }, (_, i) => ({ id: p + i })),
  edges: Array.from({ length: n - 1 }, (_, i) => ({ id: `${p}${i}-${i + 1}`, source: p + i, target: p + (i + 1) })) });
const settle = (call) => call.then((r) => ({ ok: true, nodes: r.nodes, edges: r.edges }),
  (e) => ({ ok: false, name: e instanceof Error ? e.name : String(e) }));
const start = window.__embed.heard.length;
const first = settle(el.loadGraph(chain(30, 'a')));
%s
const second = settle(el.loadGraph(chain(20, 'b')));
const results = [await first, await second];
await new Promise((done) => setTimeout(done, 500));
const heard = window.__embed.heard.slice(start).filter((h) => h.type === 'graph-load' || h.type === 'graph-error');
return { results, heard: heard.map((h) => ({ type: h.type, detail: h.detail })) };
"""


def step_overlap(page, ctx):
    name = "embed-overlap"
    expectation = ("two loadGraph calls, the second made before the first settled: the first rejects with "
                   "CancelledError, the second resolves with 20 nodes, and `document` hears one graph-load, of 20")
    # The negative control awaits the first load before it makes the second: no overlap, two loads.
    raced = page.evaluate(f"(async () => {{ const el = {embedpage.ELEMENT}; "
                          f"{OVERLAP % ('await first;' if ctx['overlap_awaits'] else '')} }})()")
    first, second = raced["results"]
    loads = [each["detail"] for each in raced["heard"] if each["type"] == "graph-load"]
    errors = [each["detail"] for each in raced["heard"] if each["type"] == "graph-error"]
    passed = (first == {"ok": False, "name": "CancelledError"} and second == {"ok": True, "nodes": 20, "edges": 19}
              and [(load["nodes"], load["edges"]) for load in loads] == [(20, 19)] and not errors)
    measured = (f"first {json.dumps(first)}, second {json.dumps(second)}; graph-load "
                f"{json.dumps([(load['nodes'], load['edges']) for load in loads])}, graph-error {json.dumps(errors)}")
    return [verdict.row(name, expectation, measured, passed)]


REFUSED = """
const start = window.__embed.heard.length;
let name = 'resolved';
try { await el.loadGraph({ nodes: 'x' }); } catch (error) { name = error instanceof Error ? error.name : String(error); }
await new Promise((done) => setTimeout(done, 300));
return { name, errors: window.__embed.heard.slice(start).filter((h) => h.type === 'graph-error').map((h) => h.detail) };
"""


def step_refused(page, ctx):
    name = "embed-load-refused"
    expectation = ("loadGraph({nodes: 'x'}) rejects, and its error's name equals the `error` of the one "
                   "graph-error `document` hears")
    refused = embedpage.on_element(page, REFUSED)
    errors = refused["errors"]
    passed = refused["name"] != "resolved" and len(errors) == 1 and errors[0]["error"] == refused["name"]
    return [verdict.row(name, expectation, f"rejected as {json.dumps(refused['name'])}; graph-error {json.dumps(errors)}", passed)]
