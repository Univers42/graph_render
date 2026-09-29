"""S3-gap rows: the background modes, the node size bounds, the animate default and console parity.

Reads the studio's store and the view's own numbers; pixels only for the canvas corner. `broken`
makes the animate row expect 10001 where the app's default is 10000.
"""
import json

from displaylib import run
from displayrows2 import BYTE_TOLERANCE, OWN_BACKGROUNDS
from navrows import row

FLAT_DARK = (0x20, 0x23, 0x2a)
PARKED_MS = 2000
ANIMATE_DEFAULT_MS = 10000


def _near(a, b):
    return all(abs(x - y) <= BYTE_TOLERANCE for x, y in zip(a, b))


def row_background(studio, broken):
    got = run(studio, """
      await d.set('appearance.theme', { name: 'dark' });
      const seen = {};
      for (const mode of ['theme', 'flat', 'aurora']) {
        const armed = window.__raf || 0;
        await d.set('appearance.background', { mode });
        const before = window.__raf || 0;
        await d.sleep(%d);
        seen[mode] = { named: d.appearance().background, corner: d.corner(), raf: (window.__raf || 0) - before, drew: before - armed };
      }
      await d.set('appearance.background', { mode: 'theme' });
      return seen;
    """ % PARKED_MS)
    theme, flat, aurora = got["theme"], got["flat"], got["aurora"]
    named = all(got[m]["named"] == m for m in got)
    corners = (_near(theme["corner"], OWN_BACKGROUNDS["dark"]) and _near(flat["corner"], FLAT_DARK)
               and not _near(aurora["corner"], OWN_BACKGROUNDS["dark"]) and not _near(aurora["corner"], FLAT_DARK))
    # The counter must see the frame a change asks for, or a 0 while parked proves nothing.
    parked = all(got[m]["raf"] == 0 for m in got) and all(got[m]["drew"] > 0 for m in ("flat", "aurora"))
    return row("s3-background", "the state names each mode; corner is theme.background / the flat colour / neither for aurora; 0 rAF callbacks parked 2 s",
               "; ".join(f"{m}: {got[m]['named']} corner {got[m]['corner']} rAF {got[m]['raf']} (change drew {got[m]['drew']})" for m in got),
               named and corners and parked)


def row_size_bounds(studio, broken):
    got = run(studio, """
      await d.set('appearance.minpx', { px: 2 });
      await d.set('appearance.maxpx', { px: 8 });
      const out = {};
      for (const factor of [0.2, 5]) {
        await d.set('appearance.scale', { factor });
        out[factor] = Array.from(d.view.radii());
      }
      await d.set('appearance.scale', { factor: 1 });
      await d.set('appearance.minpx', { px: 8 });
      const before = d.appearance();
      const refused = await d.studio.dispatch('appearance.maxpx', { px: 2 });
      out.refused = [refused.ok, refused.message || refused.error || ''];
      out.unchanged = JSON.stringify(before) === JSON.stringify(d.appearance());
      await d.set('appearance.maxpx', { px: 120 });
      await d.set('appearance.minpx', { px: 0.5 });
      return out;
    """)
    low, high = got["0.2"], got["5"]
    inside = all(2 - 1e-4 <= r <= 8 + 1e-4 for r in low + high)
    at_bounds = any(abs(r - 2) < 1e-4 for r in low) and any(abs(r - 8) < 1e-4 for r in high)
    inverted = got["refused"][0] is False and got["unchanged"]
    return row("s3-size-bounds", "min 2 / max 8 px: every radius in [2, 8], one at each bound (size 0.2 and 5); (8, 2) refused, state unchanged",
               f"0.2x {min(low):.3f}..{max(low):.3f}, 5x {min(high):.3f}..{max(high):.3f}; inverted refused {got['refused']}, unchanged {got['unchanged']}",
               inside and at_bounds and inverted)


def row_animate_default(studio, broken):
    want = ANIMATE_DEFAULT_MS + (1 if broken else 0)
    got = run(studio, """
      const a = d.studio.registry.actions.find((x) => x.id === 'appearance.animate');
      return a.params[0].value(d.studio.store.get());
    """)
    return row("s3-animate-default", f"the animate action's default duration is {want}", f"default {got}", got == want)


CONSOLE = (
    ("background", "background aurora", "appearance.background", {"mode": "aurora"}, "background"),
    ("minpx", "minpx 3", "appearance.minpx", {"px": 3}, "minRadius"),
    ("maxpx", "maxpx 9", "appearance.maxpx", {"px": 9}, "maxRadius"),
)


def row_console(studio, broken):
    got = run(studio, """
      const rows = %s;
      const out = [];
      const reset = async () => {
        await d.set('appearance.background', { mode: 'theme' });
        await d.set('appearance.maxpx', { px: 120 });
        await d.set('appearance.minpx', { px: 0.5 });
      };
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
    return row("s3-console", "each new action typed in the console leaves the same appearance as the dock's dispatch, and changes it",
               f"{len(got)} actions; differing {bad or 'none'}", not bad)
