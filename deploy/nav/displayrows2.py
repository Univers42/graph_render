"""Display rows, part two: themes, glow, the console, the animation and the dock's controls.

Pixels are read only where a row says so: one corner of the canvas for a theme, the canvas'
own bytes for glow. Everything else is the studio's state.
"""
import re
from pathlib import Path

from displaylib import run
from navrows import row

REPO = Path(__file__).resolve().parents[2]
PRESETS = REPO / "packages/graph-render/src/look/presets.ts"
# The four themes the studio owns, by their background (render/src/theme.ts, look/themes.ts).
OWN_BACKGROUNDS = {
    "dark": (0x1b, 0x1b, 0x1f), "light": (0xfb, 0xfb, 0xfc),
    "obsidian-dark": (0x1e, 0x1e, 0x1e), "obsidian-light": (0xff, 0xff, 0xff),
}
BYTE_TOLERANCE = 1
MIN_TARGET_PX = 24


def _byte(value):
    """A linear-light channel as the sRGB byte the canvas holds (IEC 61966-2-1)."""
    encoded = 12.92 * value if value <= 0.0031308 else 1.055 * value ** (1 / 2.4) - 0.055
    return int(encoded * 255 + 0.5)


def preset_backgrounds():
    """Look backgrounds by name, read out of the same table the painter draws from.

    Ponytail: a regex over presets.ts; a look written across several lines is not found and
    then shows up as "unchecked" in the row, never as a pass. "scigraphs" is the gallery copy.
    """
    text = PRESETS.read_text()
    found = re.findall(r'look\("([a-z]+)", \{\s*background: \[([\d.]+), ([\d.]+), ([\d.]+)\]', text)
    table = {name: tuple(_byte(float(v)) for v in rgb) for name, *rgb in found}
    if "gallery" in table:
        table["scigraphs"] = table["gallery"]
    return table


def row_themes(studio, broken):
    wanted = {**OWN_BACKGROUNDS, **preset_backgrounds()}
    got = run(studio, """
      const names = d.studio.registry.actions.find((a) => a.id === 'appearance.theme').params[0].choices();
      const before = d.studio.store.get().run;
      let layouts = 0, busy = 0;
      const stop = d.studio.store.subscribe(() => {
        const at = d.studio.store.get();
        if (at.run !== before) layouts += 1;
        if (at.busy.length > 0) busy += 1;
      });
      const corners = {};
      for (const name of names) { await d.set('appearance.theme', { name }); corners[name] = d.corner(); }
      stop();
      await d.set('appearance.theme', { name: 'dark' });
      return { names, corners, layouts, busy };
    """)
    wrong = [n for n in got["names"] if n in wanted and any(
        abs(a - b) > BYTE_TOLERANCE for a, b in zip(got["corners"][n], wanted[n]))]
    unknown = [n for n in got["names"] if n not in wanted]
    passed = not wrong and not unknown and got["layouts"] == 0 and got["busy"] == 0
    return row("display-themes", "every theme's background is the canvas corner, and no layout or other motor call ran",
               f"{len(got['names'])} themes; wrong {wrong or 'none'}; unchecked {unknown or 'none'}; "
               f"layout runs {got['layouts']}, busy states {got['busy']}", passed)


def row_glow(studio, broken):
    got = run(studio, """
      await d.set('appearance.glow', { on: false });
      const same = (a, b) => { let n = 0; for (let i = 0; i < a.length; i += 1) if (a[i] !== b[i]) n += 1; return n; };
      const base = Uint8ClampedArray.from(d.pixels());
      await d.set('appearance.glow', { on: true });
      const lit = Uint8ClampedArray.from(d.pixels());
      await d.set('appearance.glow', { on: false });
      const again = d.pixels();
      return { changed: same(base, lit), restored: same(base, again) };
    """)
    return row("display-glow", "glow on changes pixels; off again equals the no-glow baseline exactly",
               f"{got['changed']} bytes changed by glow, {got['restored']} differ after off",
               got["changed"] > 0 and got["restored"] == 0)


# label, console line, action id, panel args, member of appearance
CONSOLE = (
    ("arrows", "arrows off", "appearance.arrows", {"on": False}, "arrows"),
    ("fade", "fade 2", "appearance.fade", {"value": 2}, "textFade"),
    ("scale", "scale 2", "appearance.scale", {"factor": 2}, "nodeScale"),
    ("thickness", "thickness 3", "appearance.thickness", {"factor": 3}, "linkThickness"),
    ("edgestyle", "edgestyle curve", "appearance.edgestyle", {"style": "curve"}, "edgeStyle"),
    ("glow", "glow on", "appearance.glow", {"on": True}, "glow"),
    ("theme", "theme obsidian-light", "appearance.theme", {"name": "obsidian-light"}, "theme"),
)


def row_console(studio, broken):
    import json
    got = run(studio, """
      const rows = %s;
      const out = [];
      const reset = async () => { for (const name of ['dark']) await d.set('appearance.theme', { name });
        await d.set('appearance.arrows', { on: true }); await d.set('appearance.fade', { value: 0 });
        await d.set('appearance.scale', { factor: 1 }); await d.set('appearance.thickness', { factor: 1 });
        await d.set('appearance.edgestyle', { style: 'straight' }); await d.set('appearance.glow', { on: false }); };
      for (const [label, line, id, args, member] of rows) {
        await reset();
        const start = d.appearance();
        await d.studio.run(line);
        const viaConsole = d.appearance();
        await reset();
        await d.studio.dispatch(id, args);
        const viaPanel = d.appearance();
        out.push([label, JSON.stringify(viaConsole) === JSON.stringify(viaPanel), viaConsole[member] !== start[member]]);
      }
      await reset();
      return out;
    """ % json.dumps(CONSOLE))
    bad = [label for label, same, moved in got if not (same and moved)]
    return row("display-console", "each control typed in the console leaves the same appearance as the dock's dispatch, and changes it",
               f"{len(got)} controls; differing {bad or 'none'}", not bad)


def row_animate(studio, broken):
    got = run(studio, """
      const total = d.studio.store.get().meta.nodeCount;
      const at = () => [d.studio.store.get().reveal, d.view.stats().drawnNodes];
      await d.studio.dispatch('appearance.animate', { ms: 1500 });
      await d.sleep(60);
      const first = at();
      const samples = [];
      for (let i = 0; i < 12; i += 1) { await d.sleep(150); samples.push(d.studio.store.get().reveal); }
      await d.sleep(500);
      const last = at();
      await d.studio.dispatch('appearance.animate', { ms: 4000 });
      await d.sleep(1200);
      await d.studio.dispatch('appearance.animatestop', {});
      await d.sleep(350);
      const stopped = at();
      await d.sleep(800);
      const later = at();
      await d.studio.dispatch('appearance.animate', { ms: 0 });
      return { total, first, samples, last, stopped, later };
    """)
    counts = [c for c in got["samples"] if c is not None]
    ordered = all(a <= b for a, b in zip(counts, counts[1:])) and len(set(counts)) >= 3
    began = got["first"][0] is not None and got["first"][0] < got["total"] * 0.2 and got["first"][1] < got["total"]
    ended = got["last"][0] is None and got["last"][1] > 0
    partial = got["stopped"] == got["later"] and 0 < (got["stopped"][0] or 0) < got["total"]
    return row("display-animate", "starts with (almost) no node shown, reveals in ingest order to all, and cancel keeps the partial set",
               f"{got['total']} nodes; start {got['first']}; samples {counts}; end {got['last']}; "
               f"cancelled {got['stopped']} then {got['later']}", began and ordered and ended and partial)


def row_dock(studio, broken):
    got = run(studio, """
      const host = document.querySelector('graph-studio').shadowRoot;
      const dock = host.querySelector('.gs-dock, [class*=dock]');
      if (dock === null) return { missing: true };
      const small = [];
      let count = 0;
      for (const el of dock.querySelectorAll('button, input, select, [role=switch], [role=slider]')) {
        const box = el.getBoundingClientRect();
        if (box.width === 0 || box.height === 0) continue;
        count += 1;
        if (box.width < %d || box.height < %d) small.push((el.className || el.tagName) + ' ' + Math.round(box.width) + 'x' + Math.round(box.height));
      }
      return { count, small };
    """ % (MIN_TARGET_PX, MIN_TARGET_PX))
    if got.get("missing"):
        return row("display-dock-targets", "every visible dock control is at least 24x24 px", "no dock element found", False,
                   "the probe found no element matching the dock's class")
    return row("display-dock-targets", "every visible dock control is at least 24x24 px",
               f"{got['count']} controls; smaller: {got['small'] or 'none'}", got["count"] > 0 and not got["small"])
