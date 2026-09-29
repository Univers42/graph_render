"""The inferno ramp computed from source, and what the view says."""
import re
from frbase import COLORMAP, Missing, STOPS, TABLES, Unjudgeable
from frpage import dispatch, drive, settled, state

# ------------------------------------------------------- the colormap, computed from source

def stops_of():
    """The 32 pinned inferno stops, parsed out of the committed table.

    Ponytail: the block is found by the text `inferno: [` and closed by the next `]`, which
    is how the generated file is written. A table reformatted into something else makes this
    raise rather than guess — a wrong colour table would make row 11 red for the wrong
    reason, and a silent fallback to zeros would make it red for no reason at all.
    """
    text = TABLES.read_text()
    start = text.index(f"{COLORMAP}: [")
    numbers = re.findall(r"-?\d+\.\d+", text[start:text.index("]", start)])
    if len(numbers) != STOPS * 3:
        raise Unjudgeable(f"the {COLORMAP} table holds {len(numbers)} numbers, not {STOPS * 3}")
    return [float(number) for number in numbers]


def mix(a, b, f):
    return a + (b - a) * f


def sample(stops, t):
    """colour/colormap.ts locate() :52-56 and sampleColormap() :59-68, on the pinned stops."""
    scaled = min(1.0, max(0.0, t)) * (STOPS - 1)
    stop = min(STOPS - 1, int(scaled))
    nxt = min(STOPS - 1, stop + 1)
    fraction = scaled - stop
    return [mix(stops[stop * 3 + c], stops[nxt * 3 + c], fraction) for c in range(3)]


def encode(linear):
    """colour/srgb.ts srgbEncode() :24-26, with its own 0.0031308 knee (srgb.ts:12-15)."""
    x = min(1.0, max(0.0, linear))
    return 12.92 * x if x <= 0.0031308 else 1.055 * x ** (1 / 2.4) - 0.055


def byte_of(linear):
    """colour/srgb.ts byteOf() :30-32 — the 8-bit byte of one channel, rounded once."""
    return min(255, max(0, round(255 * encode(linear))))


def bytes_of(rgb):
    return tuple(byte_of(channel) for channel in rgb)


def normalised(values):
    """Each value's place between the smallest and the largest, 0 when they are all equal.

    Ponytail: this is the studio's own min/max span (look/styleOf.ts spanOf/normalised), which
    is the linear mode of colour/normalise.ts with no options. A studio that clips to
    percentiles or ranks instead would land elsewhere on purpose and row 11 would be red
    about that choice; the escape hatch is the analysis the row picks, and the values are
    read from the studio's own report so the two cannot disagree about the sample.
    """
    finite = [value for value in values if value == value and abs(value) != float("inf")]
    if not finite:
        return [0.0 for _ in values]
    low, high = min(finite), max(finite)
    if high == low:
        return [0.0 for _ in values]
    return [min(1.0, max(0.0, (value - low) / (high - low))) for value in values]


def bytes_of_colour(colour):
    """The 8-bit triple of a palette entry, whatever spelling the palette writes it in."""
    text = str(colour).strip()
    if text.startswith("#"):
        digits = text[1:]
        if len(digits) == 3:
            digits = "".join(digit * 2 for digit in digits)
        if len(digits) < 6:
            raise Unjudgeable(f"the palette entry {colour!r} is not a colour this probe can read")
        return tuple(int(digits[at:at + 2], 16) for at in (0, 2, 4))
    numbers = re.findall(r"\d+(?:\.\d+)?", text)
    if len(numbers) < 3:
        raise Unjudgeable(f"the palette entry {colour!r} is not a colour this probe can read")
    return tuple(round(float(number)) for number in numbers[:3])


# ---------------------------------------------------------------------------- what the view says

def filter_of(studio):
    return state(studio)["filter"] or {}


def visible_from_view(studio):
    """The nodes the view is drawing, read from the view's own hidden column."""
    report = state(studio)
    hidden = report["hidden"]
    if report["meta"] is None:
        raise Missing("nothing is drawn, so there is no hidden set to read")
    count = report["meta"]["nodeCount"]
    if hidden is None:
        return set(range(count))
    return {i for i in range(count) if hidden[i] == 0}


def palette_of(studio):
    report = state(studio)
    if report["palette"] is None or report["colours"] is None:
        raise Missing("the view carries no style, so its colours cannot be read")
    return report["palette"], report["colours"]


def colour_of_node(studio, index):
    palette, colours = palette_of(studio)
    slot = colours[index]
    if slot >= len(palette):
        raise Unjudgeable(f"node {index} has palette slot {slot} of {len(palette)}")
    return bytes_of_colour(palette[slot])


def reset(studio):
    """Clear every filter, so a row measures its own and not the last row's."""
    drive(studio, "unfilter")
    # Groups outlive a filter clear, and a row that finds the last row's groups measures them.
    # Ponytail: only the groups this gate adds are dropped; a group it did not add stays.
    for group in ("red", "green", "blue"):
        dispatch(studio, "dropgroup", name=group)
    left = filter_of(studio)
    still = {name: value for name, value in left.items()
             if value not in (None, "", 0, [], False)}
    if still:
        raise Unjudgeable(f"`unfilter` left the filter as {still}")
    settled(studio, 2.0)


def layout_calls(studio):
    calls = state(studio)["layoutCalls"]
    if calls is None:
        raise Missing("the studio counts no layout calls, so this row has nothing to measure")
    return calls
