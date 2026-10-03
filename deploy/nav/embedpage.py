"""What the embed gate reads from `app/embed.html`, and the faults its negative controls inject.

The page keeps everything it heard on `window.__embed` (`app/src/embed.ts`); the element sits in
a shadow root of the page's own, so every probe reaches it through `#frame`'s shadow root.

Each fault is a script run before the page's own (`Page.addScriptToEvaluateOnNewDocument`), so
nothing in `app/` or `packages/` carries a switch for a gate to flip, and a fault cannot mask a
real regression: it breaks one thing the browser does, and the row that watches that thing must
go red. None of them is a product knob.
"""
import time

ELEMENT = "document.querySelector('#frame').shadowRoot.querySelector('graph-studio')"

# How long the page has to define the element, fetch the fixture and draw it. It builds the
# fixture, opens a motor session and lays it out, all on load: the load smoke gate's cap.
LOAD_CAP_S = 90.0

# The page's state and the studio's, read once. `studio` is null while there is no studio; its
# fields are the ones `smokerows.row_no_store_error`, `row_no_overlay` and `row_drew_nodes` read.
HEALTH = f"""
(() => {{
  const embed = window.__embed;
  const host = {ELEMENT};
  const page = embed === undefined ? null : {{ state: embed.state, heard: embed.heard, asked: embed.asked, loadCalledAt: embed.loadCalledAt }};
  if (host === null || !host.studio) return {{ page, studio: null }};
  const at = host.studio.store.get();
  const alert = host.shadowRoot.querySelector('.gs-alert');
  return {{ page, studio: {{
    error: at.error, meta: at.meta !== null, busy: at.busy.length,
    nodes: host.view === null ? 0 : host.view.stats().nodes,
    banner: alert === null ? null : alert.textContent,
  }} }};
}})()
"""

# A node on the canvas and the page point it is drawn at: the hit nearest the centre that no
# panel covers, by the view's own hit-test and the shadow root's own `elementFromPoint`.
NODE_POINT = f"""
(() => {{
  const host = {ELEMENT};
  if (host === null || !host.view || !host.studio) return null;
  const meta = host.studio.store.get().meta;
  const canvas = host.shadowRoot.querySelector('.gs-canvas');
  if (meta === null || canvas === null) return null;
  const box = canvas.getBoundingClientRect();
  let best = null;
  for (let y = 10; y < box.height - 10; y += 6) {{
    for (let x = 10; x < box.width - 10; x += 6) {{
      const node = host.view.pick({{ x, y }});
      const far = (x - box.width / 2) ** 2 + (y - box.height / 2) ** 2;
      if (node < 0 || (best !== null && far >= best.far)) continue;
      if (host.shadowRoot.elementFromPoint(box.left + x, box.top + y) !== canvas) continue;
      best = {{ x: box.left + x, y: box.top + y, node, far }};
    }}
  }}
  if (best === null) return null;
  const id = meta.ids[best.node];
  const other = meta.ids.find((each) => each !== id);
  return {{ x: best.x, y: best.y, id, other: other === undefined ? null : other }};
}})()
"""

FIRST_POSITION = f"(() => {{ const p = {ELEMENT}.view.position(0); return [p.x, p.y]; }})()"

HEARD_COUNT = "window.__embed.heard.length"


def heard_since(page, start):
    """Every event the page's `document` listener heard from index `start` on."""
    return page.evaluate(f"window.__embed.heard.slice({int(start)})")


def on_element(page, body):
    """`body` run as a function of the element `el`; its value, awaited."""
    return page.evaluate(f"(async () => {{ const el = {ELEMENT}; {body} }})()")


def wait_loaded(page):
    """The last HEALTH read once the page has answered, the studio failed, or the cap ran out.

    Never raises: a page that failed to load is the thing the rows measure.
    """
    deadline = time.monotonic() + LOAD_CAP_S
    health = None
    while time.monotonic() < deadline:
        page.watch_workers()
        health = page.evaluate(HEALTH)
        page_state = (health or {}).get("page") or {}
        studio = (health or {}).get("studio") or {}
        if page_state.get("state", "pending") != "pending" or studio.get("error") is not None:
            return health
        time.sleep(0.2)
    return health


def settle_drawing(page, cap=12.0, quiet=0.4):
    """Wait for the first node to stop moving, as `drive.Studio.settle_drawing` does for index.html.

    Caveat: node 0 only. A layout whose node 0 rests while others still travel reads as settled;
    the rows that need a still node re-find it by hit-test, so they read where it is now.
    """
    deadline = time.monotonic() + cap
    was = None
    while time.monotonic() < deadline:
        time.sleep(quiet)
        now = page.evaluate(FIRST_POSITION)
        if was is not None and abs(now[0] - was[0]) + abs(now[1] - was[1]) < 1e-6:
            return
        was = now


HOST_EVENTS = "['graph-load', 'node-select', 'node-open', 'node-hover', 'graph-error']"

# The faults, one per row they break, each named by its `--break` run.
FAULTS = {
    # Verdict 3's break: the element gets `remember`, so it uses the page's storage and reopens
    # the last graph by itself.
    "remember": """
(() => {
  const made = Document.prototype.createElement;
  Document.prototype.createElement = function (name, options) {
    const element = made.call(this, name, options);
    if (String(name).toLowerCase() === 'graph-studio') element.setAttribute('remember', '');
    return element;
  };
})();
""",
    # Verdict 10's break: each host event is sent again with `composed: false`, so it stops at the
    # page's shadow root and the `document` listener never hears it.
    "composed": f"""
(() => {{
  const types = new Set({HOST_EVENTS});
  const send = EventTarget.prototype.dispatchEvent;
  EventTarget.prototype.dispatchEvent = function (event) {{
    if (!(event instanceof CustomEvent) || !types.has(event.type) || !event.composed) return send.call(this, event);
    return send.call(this, new CustomEvent(event.type, {{ detail: event.detail, bubbles: event.bubbles, composed: false }}));
  }};
}})();
""",
    # Verdict 11's three breaks, one per trigger: the browser stops the gesture before the studio.
    "dblclick": "window.addEventListener('dblclick', (event) => event.stopImmediatePropagation(), true);",
    "enter": "window.addEventListener('keydown', (event) => { if (event.key === 'Enter') event.stopImmediatePropagation(); }, true);",
    "open": "window.addEventListener('click', (event) => event.stopImmediatePropagation(), true);",
    # Verdict 12's break: the element's constructor no longer sees the `resolve` the page set
    # before the upgrade, so the page's own property shadows the accessor and is never called.
    "upgrade": """
(() => {
  const own = Object.hasOwn;
  Object.hasOwn = (target, key) => (key === 'resolve' && target instanceof HTMLElement ? false : own(target, key));
})();
""",
}
