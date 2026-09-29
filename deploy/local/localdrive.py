"""What the local-graph and settings rows need on top of the navigation probe's Studio.

Actions go in through the studio's own dispatch (there is no menu to click yet: the node
context menu is S2's), keys go in as CDP key events, and everything read back is read from
the store or the view the served page is using. The independent parts (the BFS, the fixture
edges) are computed here from the served fixture file, never from the page's own adjacency.
"""
import json
import time

import cdp
from drive import SETTLE_S, Studio

HOST = "document.querySelector('graph-studio')"
# Ponytail: a layout that has not stopped moving would make a drawn-node count a count of a
# drawing in flight; the pause is a fixed one and a slower host needs a longer one.
ARRIVE_S = 1.5


class LocalStudio(Studio):
    def dispatch(self, action, raw=None):
        """Runs one action through the studio; returns {ok, message, digest}."""
        call = f"{HOST}.studio.dispatch({json.dumps(action)}, {json.dumps(raw or {})})"
        entry = self.page.evaluate(f"{call}.then(e => ({{ ok: e.ok, message: e.message, digest: e.digest }}))")
        time.sleep(SETTLE_S)
        return entry

    def settings(self):
        return self.page.evaluate(f"JSON.parse(JSON.stringify({HOST}.studio.store.get().settings))")

    def ids(self):
        return self.page.evaluate(f"{HOST}.studio.store.get().meta.ids")

    def stats(self):
        return self.page.evaluate(f"{HOST}.view.stats()")

    def arrive(self):
        time.sleep(ARRIVE_S)
        return self.stats()

    def open_source(self, path):
        entry = self.dispatch("source.fixture", {"path": path})
        if not entry["ok"]:
            raise cdp.CdpError(f"source.fixture {path}: {entry['message']}")
        return self.arrive()

    def visible_ids(self, root, depth, kinds):
        """The ids `view.local` reports, mapped through the store's own id column."""
        raw = {"id": root, "depth": depth, **kinds}
        entry = self.dispatch("view.local", raw)
        if not entry["ok"]:
            raise cdp.CdpError(f"view.local {raw}: {entry['message']}")
        ids = self.ids()
        return sorted(ids[at] for at in json.loads(entry["digest"]))

    def fixture_edges(self, path):
        """The served fixture's own edge list, as (source, target) pairs."""
        document = self.page.evaluate(f"fetch(new URL('fixtures/{path}', document.baseURI)).then(r => r.json())")
        return [(edge["source"], edge["target"]) for edge in document["edges"]]

    def press(self, key, code, vk, shift=False):
        for kind in ("rawKeyDown", "keyUp"):
            self.page.call("Input.dispatchKeyEvent", {
                "type": kind, "key": key, "code": code, "windowsVirtualKeyCode": vk,
                "nativeVirtualKeyCode": vk, "modifiers": 8 if shift else 0,
                **({"text": key} if kind == "rawKeyDown" and len(key) == 1 else {}),
            })
        time.sleep(SETTLE_S)

    def capture_saves(self):
        """Blobs the studio offers for download are kept in the page, and the click is dropped."""
        self.page.evaluate("""
        (() => {
          window.__saved = [];
          const make = URL.createObjectURL.bind(URL);
          URL.createObjectURL = (blob) => { window.__saved.push(blob); return make(blob); };
          HTMLAnchorElement.prototype.click = function () {};
        })()
        """)

    def last_saved(self):
        return self.page.evaluate("window.__saved[window.__saved.length - 1].text()")


def walk(edges, root, depth, kinds):
    """Breadth-first, level by level, over edge pairs: the probe's own answer, not the page's."""
    both = kinds.get("neighbours") or not (kinds.get("incoming") or kinds.get("outgoing"))
    around = {}
    for source, target in edges:
        if both or kinds.get("outgoing"):
            around.setdefault(source, set()).add(target)
        if both or kinds.get("incoming"):
            around.setdefault(target, set()).add(source)
    seen, frontier = {root}, {root}
    for _ in range(depth):
        frontier = {other for node in frontier for other in around.get(node, ())} - seen
        seen |= frontier
    return sorted(seen)
