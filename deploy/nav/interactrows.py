"""The interaction rows: hover, click, box select, node drag and the node menu.

Every row is judged by what the served app reports (the view's own opacity, selection and node
positions, the store's clipboard) after real CDP mouse input; nothing is dispatched through the
studio's API. `--break` makes the hover row expect 0.13 where the app fades to 0.12.
"""
import time

from drive import SETTLE_S
from navrows import row

FADE_S = 0.6
SHIFT = 8
DIM = 0.12
DIM_BROKEN = 0.13
OPACITY_TOLERANCE = 0.005
FOLLOW_TOLERANCE = 0.5
DRAG_PX = 150
MENU_LABELS = ("focus", "pin", "hide", "copy")


def evaluate(studio, body):
    return studio.page.evaluate(f"(() => {{ const host = document.querySelector('graph-studio'); "
                                f"const view = host.view; const meta = host.studio.store.get().meta; {body} }})()")


def mouse(studio, kind, at, **extra):
    studio.page.call("Input.dispatchMouseEvent", {"type": kind, "x": at[0], "y": at[1], **extra})


def hover(studio, at):
    mouse(studio, "mouseMoved", (at[0] - 12, at[1] - 12), button="none", buttons=0)
    mouse(studio, "mouseMoved", at, button="none", buttons=0)
    time.sleep(FADE_S)


def click_with(studio, at, modifiers=0, button="left", buttons=1):
    mouse(studio, "mouseMoved", at, button="none", buttons=0)
    mouse(studio, "mousePressed", at, button=button, buttons=buttons, clickCount=1, modifiers=modifiers)
    mouse(studio, "mouseReleased", at, button=button, buttons=0, clickCount=1, modifiers=modifiers)
    time.sleep(SETTLE_S)


def nodes_on_screen(studio, count):
    """Screen points that hit distinct nodes, by the view's own hit-test."""
    return evaluate(studio, f"""
      const found = new Map();
      for (let y = 60; y < 840 && found.size < {count}; y += 5) {{
        for (let x = 60; x < 1340 && found.size < {count}; x += 5) {{
          const n = view.pick({{ x, y }});
          if (n >= 0 && !found.has(n)) found.set(n, [x, y]);
        }}
      }}
      return Array.from(found, ([node, at]) => ({{ node, at }}));""")


def row_hover(studio, expected):
    name, text = "int-hover", "hover fades everything but the node and its neighbours to 0.12 and labels them"
    hit = nodes_on_screen(studio, 1)
    if not hit:
        return row(name, text, "the probe found no node", False, "no node under the hit-test")
    node, at = hit[0]["node"], hit[0]["at"]
    hover(studio, at)
    read = evaluate(studio, f"""
      const lit = []; const dim = []; const stray = [];
      for (let i = 0; i < meta.nodeCount; i += 1) {{
        const o = view.opacity('node', i);
        if (o === 1) lit.push(i); else if (Math.abs(o - {expected}) <= {OPACITY_TOLERANCE}) dim.push(i); else stray.push(o);
      }}
      let edgeLit = 0; let edgeDim = 0; let edgeStray = 0;
      for (let e = 0; e < view.stats().edges; e += 1) {{
        const o = view.opacity('edge', e);
        if (o === 1) edgeLit += 1; else if (Math.abs(o - {expected}) <= {OPACITY_TOLERANCE}) edgeDim += 1; else edgeStray += 1;
      }}
      const labelled = new Set(view.labelled());
      const cam = view.camera();
      const onScreen = (p) => {{ const x = p.x * cam.scale + cam.x; const y = p.y * cam.scale + cam.y;
        return x >= 0 && x <= 1400 && y >= 0 && y <= 900; }};
      return {{ lit, dim: dim.length, stray: stray.length, edgeLit, edgeDim, edgeStray,
        unlabelled: lit.filter((n) => !labelled.has(n) && onScreen(view.position(n))).length }};""")
    ok = (node in read["lit"] and len(read["lit"]) >= 2 and read["dim"] > 0 and read["stray"] == 0
          and read["edgeLit"] > 0 and read["edgeDim"] > 0 and read["edgeStray"] == 0 and read["unlabelled"] == 0)
    return row(name, text, f"node {node}: {len(read['lit'])} lit, {read['dim']} at {expected}, {read['stray']} strays; "
               f"edges {read['edgeLit']} lit, {read['edgeDim']} dim; {read['unlabelled']} lit unlabelled on screen", ok)


def row_click(studio):
    name, text = "int-click", "click selects the node and the Inspector shows it"
    hit = nodes_on_screen(studio, 1)
    if not hit:
        return row(name, text, "the probe found no node", False, "no node under the hit-test")
    click_with(studio, hit[0]["at"])
    state = evaluate(studio, "return { selected: host.studio.store.get().selected, "
                             "inspector: host.shadowRoot.textContent.length > 0 };")
    return row(name, text, f"selected {state['selected']} for node {hit[0]['node']}",
               state["selected"] == hit[0]["node"] and state["inspector"])


def row_shift_click(studio):
    name, text = "int-shift-click", "shift-click adds a second node to the selection"
    hit = nodes_on_screen(studio, 2)
    if len(hit) < 2:
        return row(name, text, "fewer than two nodes hit", False, "the probe found fewer than two nodes")
    click_with(studio, hit[0]["at"])
    click_with(studio, hit[1]["at"], modifiers=SHIFT)
    selection = evaluate(studio, "return Array.from(view.selection());")
    return row(name, text, f"selection {selection}", set(selection) == {hit[0]["node"], hit[1]["node"]})


def row_box(studio):
    name, text = "int-box", "shift+drag on the background selects exactly the nodes inside the box"
    click_with(studio, (700, 450))
    box = (300, 200, 900, 700)
    mouse(studio, "mouseMoved", (box[0], box[1]), button="none", buttons=0)
    mouse(studio, "mousePressed", (box[0], box[1]), button="left", buttons=1, clickCount=1, modifiers=SHIFT)
    for step in range(1, 21):
        mouse(studio, "mouseMoved", (box[0] + (box[2] - box[0]) * step / 20, box[1] + (box[3] - box[1]) * step / 20),
              button="left", buttons=1, modifiers=SHIFT)
    mouse(studio, "mouseReleased", (box[2], box[3]), button="left", buttons=0, clickCount=1, modifiers=SHIFT)
    time.sleep(SETTLE_S)
    got = evaluate(studio, f"""
      const cam = view.camera(); const inside = [];
      for (let i = 0; i < meta.nodeCount; i += 1) {{
        const p = view.position(i); const x = p.x * cam.scale + cam.x; const y = p.y * cam.scale + cam.y;
        if (x >= {box[0]} && x <= {box[2]} && y >= {box[1]} && y <= {box[3]}) inside.push(i);
      }}
      return {{ inside, selection: Array.from(view.selection()) }};""")
    same = sorted(got["inside"]) == sorted(got["selection"])
    return row(name, text, f"{len(got['inside'])} inside, {len(got['selection'])} selected", same and len(got["inside"]) > 0)


def row_drag_node(studio):
    name, text = "int-node-drag", "dragging a node 150 px puts it under the pointer and its edge follows"
    hit = nodes_on_screen(studio, 1)
    if not hit:
        return row(name, text, "the probe found no node", False, "no node under the hit-test")
    node, (x, y) = hit[0]["node"], hit[0]["at"]
    click_with(studio, (700, 450))
    mouse(studio, "mouseMoved", (x, y), button="none", buttons=0)
    mouse(studio, "mousePressed", (x, y), button="left", buttons=1, clickCount=1)
    for step in range(1, 21):
        mouse(studio, "mouseMoved", (x + DRAG_PX * step / 20, y), button="left", buttons=1)
    mouse(studio, "mouseReleased", (x + DRAG_PX, y), button="left", buttons=0, clickCount=1)
    time.sleep(SETTLE_S)
    got = evaluate(studio, f"""
      const cam = view.camera(); const p = view.position({node});
      const canvas = host.shadowRoot.querySelector('canvas'); const box = canvas.getBoundingClientRect();
      return {{ x: p.x * cam.scale + cam.x + box.left, y: p.y * cam.scale + cam.y + box.top }};""")
    miss = ((got["x"] - (x + DRAG_PX)) ** 2 + (got["y"] - y) ** 2) ** 0.5
    return row(name, text, f"node {node} {miss:.2f} px from the pointer", miss <= FOLLOW_TOLERANCE)


def menu_button(studio, label):
    return studio.page.evaluate(f"""
      (() => {{
        const b = Array.from(document.querySelector('graph-studio').shadowRoot.querySelectorAll('[role=menuitem]'))
          .find((e) => e.textContent.toLowerCase().includes('{label}'));
        if (!b) return null; const r = b.getBoundingClientRect();
        return [r.left + r.width / 2, r.top + r.height / 2];
      }})()""")


def open_menu(studio, at):
    click_with(studio, at, button="right", buttons=2)


def row_menu(studio):
    name, text = "int-menu", "the node menu offers focus, pin, hide and copy id; copy id writes the id to the clipboard state"
    hit = nodes_on_screen(studio, 1)
    if not hit:
        return row(name, text, "the probe found no node", False, "no node under the hit-test")
    node, at = hit[0]["node"], hit[0]["at"]
    open_menu(studio, at)
    offered = [label for label in MENU_LABELS if menu_button(studio, label) is not None]
    where = menu_button(studio, "copy")
    if where is not None:
        click_with(studio, tuple(where))
    got = evaluate(studio, "return { clip: host.studio.store.get().clipboard, ids: meta.ids };")
    return row(name, text, f"offered {offered}, clipboard {got['clip']!r}",
               len(offered) == len(MENU_LABELS) and got["clip"] == got["ids"][node])


def row_menu_hide(studio):
    name, text = "int-menu-hide", "Hide in the node menu removes the node from picking"
    hit = nodes_on_screen(studio, 1)
    if not hit:
        return row(name, text, "the probe found no node", False, "no node under the hit-test")
    node, at = hit[0]["node"], hit[0]["at"]
    open_menu(studio, at)
    where = menu_button(studio, "hide")
    if where is None:
        return row(name, text, "no Hide entry", False)
    click_with(studio, tuple(where))
    still = evaluate(studio, f"return view.pick({{ x: {at[0]}, y: {at[1]} }});")
    return row(name, text, f"pick at {at} gives {still} after hiding node {node}", still != node)


def run_rows(studio, broken):
    studio.focus_page()
    return [row_hover(studio, DIM_BROKEN if broken else DIM), row_click(studio), row_shift_click(studio),
            row_box(studio), row_drag_node(studio), row_menu(studio), row_menu_hide(studio)]
