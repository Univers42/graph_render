"""Rows of the forces gate. Each returns a verdict dict; input goes in as real CDP key and mouse events."""
import time

LABELS = ["Center force", "Repel force", "Link force", "Link distance"]
ROOT = "document.querySelector('graph-studio').shadowRoot"
REASON = "live forces need the motor session (force-wasm)"


def row(name, expectation, measured, ok):
    return {"row": name, "expectation": expectation, "measured": measured, "verdict": "PASS" if ok else "FAIL"}


def read_panel(studio):
    return studio.page.evaluate(f"""
    (() => {{
      const panel = {ROOT}.getElementById('gs-dock-forces');
      if (panel === null) return null;
      const ranges = [...panel.querySelectorAll('input[type=range]')];
      const reason = panel.querySelector('.gs-reason');
      return {{
        labels: ranges.map((r) => (r.closest('label')?.querySelector('.gs-field-label')?.textContent ?? '')),
        described: ranges.every((r) => r.getAttribute('aria-describedby') === (reason ? reason.id : null)),
        reason: reason === null ? null : reason.textContent,
      }};
    }})()
    """)


def open_section(studio):
    studio.page.evaluate(f"""
    (() => {{
      const head = {ROOT}.getElementById('gs-dock-forces-head');
      if (head !== null && head.getAttribute('aria-expanded') !== 'true') head.click();
    }})()
    """)
    time.sleep(0.3)


def tab_stops(studio, count):
    """Focus the section's head, press Tab `count` times, return the type of each stop reached."""
    studio.page.evaluate(f"{ROOT}.getElementById('gs-dock-forces-head').focus()")
    seen = []
    for _ in range(count):
        studio.key("Tab", "Tab", 9)
        seen.append(studio.page.evaluate(f"""
        (() => {{ const a = {ROOT}.activeElement; return a === null ? '' : a.tagName + ':' + (a.type ?? ''); }})()
        """))
    return seen


def row_panel(studio):
    panel = read_panel(studio)
    return row("panel-exists", "the Forces section is in the dock", "present" if panel else "absent", panel is not None)


def row_sliders(studio):
    panel = read_panel(studio) or {"labels": [], "described": False}
    return row("sliders-labelled", f"four range inputs labelled {LABELS}", str(panel["labels"]), panel["labels"] == LABELS)


def row_tab(studio):
    if read_panel(studio) is None:
        return row("sliders-tab", "Tab reaches four range inputs while disabled", "no panel", False)
    stops = tab_stops(studio, 4)
    return row("sliders-tab", "four Tab presses from the header land on four range inputs", str(stops), stops == ["INPUT:range"] * 4)


def row_reason(studio):
    panel = read_panel(studio) or {"reason": None, "described": False}
    ok = panel["reason"] == REASON and panel["described"]
    return row("disabled-reason", f"the reason `{REASON}` is shown and every slider points at it", str(panel), ok)


def row_view_drag(studio):
    node = studio.find_node()
    if node is None:
        return {"row": "view-drag", "expectation": "a view-only drag moves the node", "measured": "no node found",
                "verdict": "NOT-RUN", "why": "the view's hit-test found no node"}
    index = studio.page.evaluate(f"document.querySelector('graph-studio').view.pick({{x: {node[0]}, y: {node[1]}}})")
    read = f"document.querySelector('graph-studio').view.position({index})"
    before = studio.page.evaluate(read)
    studio.drag(node, 60, 40)
    studio.settle()
    after = studio.page.evaluate(read)
    moved = abs(after["x"] - before["x"]) + abs(after["y"] - before["y"])
    return row("view-drag", "a 60x40 px drag on a node moves that node", f"{before} -> {after}", moved > 1)


def run_rows(studio, broken):
    if broken:
        studio.page.evaluate(f"{ROOT}.getElementById('gs-dock-forces')?.remove()")
    else:
        open_section(studio)
    return [row_panel(studio), row_sliders(studio), row_tab(studio), row_reason(studio), row_view_drag(studio)]
