"""The parity gate's expectations, computed from the pinned reference rather than typed.

The background and the node fill are the two values the gate compares pixels against, and
both are derived here from the sources the studio also derives them from: the linear
triple the gallery specification carries, and the pinned matplotlib table the colormap
ramps are generated from. A hex that was typed into this file would only prove that the
harness agrees with itself.

  05-reproducible-pipeline.qmd:121 (inferno), :132-137 (edge), :139-145 (background)
  SciGraphs/core/scigraphs_core/coloring/colormaps.py:329-335 (32 stops from linspace)
  matplotlib _cm_listed.py:260 (256 listed rows, sampled the way Colormap.__call__ does)
"""

import re

# The gallery's own world colour, linear (05-reproducible-pipeline.qmd:139-145). Blender's
# Standard view transform (pipeline-schema.qmd:156, 05-reproducible-pipeline.qmd:161) turns
# it into (53, 53, 60); the reference figure's modal 16-bit corner value is (52.5, 52.5,
# 59.9) in 8-bit units, so the two agree inside TOLERANCE.
BACKGROUND = (0.035, 0.035, 0.045)

# The colormap the gallery asks for (05-reproducible-pipeline.qmd:121).
COLORMAP = "inferno"

# 43 of the 77 nodes have a betweenness of exactly 0. RANK gives a tie group its mid-rank,
# (start + end - 1) / 2 (colormaps.py:462-469), divided by n - 1 (colormaps.py:527-530):
# (0 + 42) / 2 = 21, so t = 21 / 76. The ramp stops are linspace samples (colormaps.py:329-335),
# placed at i / (count - 1) and interpolated LINEAR (ui/coloring/functions.py:250-270, :408).
# Ponytail: t = 20 / 76 would give #A24BAF, the hex the task statement writes; this file
# follows the code, and the figure's lit node centres read (168, 93, 178), whose green sits
# 14 above the ramp because Cycles lighting is added on top of the base colour.
FILL_T = 21 / 76

# Blender's ColorRamp accepts 32 stops at most, and SciGraphs asks for exactly that.
STOPS = 32
ROWS = 256

# How far a sampled pixel may sit from the expected one, per channel.
TOLERANCE = 2

# The hexes the task statement writes down. #34343C is within TOLERANCE of the computed
# background; #A24BAF is the ramp at 20 / 76, one rank below the tie's mid-rank. The gate
# compares against the computed value and the report prints both.
SPEC_BACKGROUND = "#34343C"
SPEC_FILL = "#A24BAF"

# The eighteen names legible in the gallery figure
# (SciGraphs/docs/assets/gallery/fig6_labels_lesmiserables.png, spec lines 70-72).
# Reported, never asserted: the GEXF behind the figure is in neither tree, so node order,
# and with it positions and occlusion, differ (docs/decisions/scigraphs-reference-fixture.md).
FIG6 = (
    "MmeBurgon", "Mabeuf", "Anzelma", "Bossuet", "Marius", "Fauchelevent", "Javert",
    "Cosette", "MmePontmercy", "Valjean", "Labarre", "Marguerite", "Gervais",
    "MlleBaptistine", "Myriel", "Geborand", "Champtercier", "Napoleon",
)


def encode(linear):
    """The sRGB transfer function, IEC 61966-2-1, and its own knee (colour/srgb.ts:20-22)."""
    if linear <= 0.0031308:
        return 12.92 * linear
    return 1.055 * linear ** (1 / 2.4) - 0.055


def byte_of(linear):
    """The 8-bit byte of one linear channel, rounded once (colour/srgb.ts:27-29)."""
    return min(255, max(0, round(255 * encode(linear))))


def bytes_of(rgb):
    return tuple(byte_of(channel) for channel in rgb)


def hex_of(rgb):
    """The CSS hex of a linear triple, the way the studio spells it."""
    return hex_bytes(bytes_of(rgb))


def hex_bytes(channels):
    """The hex of three bytes, as a pixel reads back out of the screenshot."""
    return "#" + "".join(f"{channel:02X}" for channel in channels)


def listed_rows(text, name):
    """The 256 listed rows of one colormap, as the pinned table writes them."""
    start = text.index(f"_{name}_data = [")
    end = text.index("\n_", start + 1)
    rows = [[float(part) for part in row.split(",")] for row in re.findall(r"\[([^\][]*)\]", text[start:end])]
    if len(rows) != ROWS:
        raise ValueError(f"_{name}_data has {len(rows)} rows, not {ROWS}")
    return rows


def ramp(rows):
    """The 32 stops of a 256-row table, at the rows matplotlib's own sampling lands on."""
    return [rows[min(ROWS - 1, int(k / (STOPS - 1) * ROWS))] for k in range(STOPS)]


def sample(stops, t):
    """The colour of a normalised value, interpolated across the 32 stops (colormap.ts:41-52)."""
    scaled = min(1.0, max(0.0, t)) * (STOPS - 1)
    at = min(STOPS - 1, int(scaled))
    up = min(STOPS - 1, at + 1)
    here, there = stops[at], stops[up]
    return [here[c] + (there[c] - here[c]) * (scaled - at) for c in range(3)]


def load(reference):
    """The gate's expected values, from the pinned table at `reference`."""
    rows = listed_rows(open(reference).read(), COLORMAP)
    return {
        "background": bytes_of(BACKGROUND),
        "fill": bytes_of(sample(ramp(rows), FILL_T)),
        "source": reference,
    }
