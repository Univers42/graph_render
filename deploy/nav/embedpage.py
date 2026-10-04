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

# The document the page asks for, by the name it asks it under. `embed.py` serves the same path
# from disk, so a fault that serves another document speaks about the file the rows measured.
FIXTURE_NAME = "clustered.json"

# How long the page has to define the element, fetch the fixture and draw it. It builds the
# fixture, opens a motor session and lays it out, all on load: the load smoke gate's cap.
# Caveat: a page that has not answered by then is read as it stands, not as a timeout, so a slow
# host (a cold wasm, a loaded machine) reads as a half-drawn studio and every row fails on what it
# saw. The way out is a warm `app/public/` from `scripts/studio.sh build`, or a larger cap here —
# no row may wait longer than this on its own.
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

# The harness's own record of the host events, installed over CDP before the page's scripts. It is
# not the page's: `app/src/embed.ts` also keeps what it heard, and a page that recorded
# `composed: true` for an event it never composed would pass a row that read its own notes.
# `bubbles`, `composed` and `frozen` are read here, off the browser's own `Event` and off the
# detail the browser handed us (`embedrows.step_composed`).
HARNESS = """
(() => {
  if (window.__harness !== undefined) return;
  const frozen = (value) => {
    if (typeof value !== 'object' || value === null) return true;
    if (!Object.isFrozen(value)) return false;
    return Object.values(value).every(frozen);
  };
  const harness = { heard: [] };
  for (const type of ['graph-load', 'node-select', 'node-open', 'node-hover', 'graph-error']) {
    document.addEventListener(type, (event) => {
      if (!(event instanceof CustomEvent)) return;
      harness.heard.push({ type, detail: event.detail, bubbles: event.bubbles, composed: event.composed,
        frozen: frozen(event.detail) });
    });
  }
  window.__harness = harness;
})();
"""


def heard_since(page, start):
    """Every event the page's `document` listener heard from index `start` on."""
    return page.evaluate(f"window.__embed.heard.slice({int(start)})")


def install_harness(page):
    """Put the harness's own `document` listener in place, before the page's first navigation."""
    page.call("Page.addScriptToEvaluateOnNewDocument", {"source": HARNESS})


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

# What a fault that acts on the studio waits for. Every fault runs before the page's own scripts,
# so the element is defined a long time after the fault starts, and a fault that acted before the
# page's own load would simply be overwritten by it. `after(read, act)` polls `read` until it
# answers, then acts once.
WAITED = """
const after = (read, act) => {
  const tick = () => {
    const at = read();
    if (at === null) { setTimeout(tick, 50); return; }
    act(at);
  };
  tick();
};
const studioElement = () => {
  const frame = document.getElementById('frame');
  const host = frame === null || frame.shadowRoot === null ? null : frame.shadowRoot.querySelector('graph-studio');
  return host === null || !host.studio ? null : host;
};
const loaded = () => ((window.__embed || {}).state === 'pending' || studioElement() === null ? null : studioElement());
"""

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
    # `embed-load-refused`'s own break (verdict 7): the `graph-error` says a different name from
    # the rejection, which is the one claim that row makes and the only way to break it. Kept out
    # of `composed`, where the events never arrive at all and every other assertion is untested.
    "name": """
(() => {
  const send = EventTarget.prototype.dispatchEvent;
  EventTarget.prototype.dispatchEvent = function (event) {
    if (!(event instanceof CustomEvent) || event.type !== 'graph-error') return send.call(this, event);
    const detail = { error: 'RenamedError', message: event.detail.message };
    return send.call(this, new CustomEvent(event.type, { detail, bubbles: event.bubbles, composed: event.composed }));
  };
})();
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
    # The task-2 regression, injected (verdict 7): a `loadGraph` made while another call is still
    # in flight waits for that one to settle instead of overtaking it, so nothing is ever
    # superseded. `embed-overlap-reentrant` then reads a first call that resolved with its own
    # counts, which is exactly what the token in `studio/pipeline.ts` is there to prevent.
    "supersede": """
(() => {
  const define = customElements.define.bind(customElements);
  let inFlight = 0;
  customElements.define = (name, ctor, options) => {
    if (name === 'graph-studio') {
      const inner = ctor.prototype.loadGraph;
      ctor.prototype.loadGraph = function (doc) {
        if (inFlight === 0) {
          inFlight += 1;
          return inner.call(this, doc).finally(() => { inFlight -= 1; });
        }
        return new Promise((done, fail) => {
          const later = () => {
            if (inFlight > 0) { setTimeout(later, 20); return; }
            inner.call(this, doc).then(done, fail);
          };
          later();
        });
      };
    }
    return define(name, ctor, options);
  };
})();
""",
    # `embed-no-store-error`'s own break: a store error with nothing behind it. `note` is the
    # studio's own way of being told something went wrong, so the banner row reads one too.
    "store-error": WAITED + """
after(loaded, (el) => el.studio.note('embed gate break: a failure the studio did not have'));
""",
    # `embed-no-overlay`'s own break: the banner a user would read, with no error behind it.
    "overlay": WAITED + """
after(loaded, (el) => {
  const banner = document.createElement('div');
  banner.className = 'gs-alert';
  banner.textContent = 'embed gate break: a banner the store never asked for';
  el.shadowRoot.prepend(banner);
});
""",
    # `embed-drew-nodes`' own break, and `embed-host-load`'s with it: the page is handed a document
    # with nothing in it, so the view draws no node and the call does not answer with the counts of
    # the fixture the gate served. One fault, two rows, each red on its own claim.
    "empty-document": f"""
(() => {{
  const fetch = window.fetch.bind(window);
  window.fetch = (input, init) => {{
    const url = typeof input === 'string' ? input : ((input && input.url) || '');
    if (!url.includes('{FIXTURE_NAME}')) return fetch(input, init);
    return Promise.resolve(new Response('{{"nodes":[],"edges":[]}}',
      {{ status: 200, headers: {{ 'content-type': 'application/json' }} }}));
  }};
}})();
""",
}