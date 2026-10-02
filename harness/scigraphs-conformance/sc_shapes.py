"""The shapes: one SVG per name, on the two fixtures a reader can see.

**Two fixtures, chosen for what they show rather than for how many there are.** `lesmis` is
SciGraphs' own gallery graph — 77 nodes, connected, the shape everyone recognises — and
`tree-balanced` is a 15-node rooted tree, where a force layout's answer is either a tree or it
is not, and nothing about node 40's degree confuses the reading. A matrix whose verdict is
"different shape" is only worth as much as the picture it was read from, so the pictures are
drawn from those two and not from a gate model.

Each SVG is three panels: SciGraphs, the motor, and the motor Procrustes-aligned over
SciGraphs. The third is the one that decides anything — it removes translation, uniform scale
and rotation, so what is left in it is the drawing's shape, and a mirrored one stays visibly
apart because Procrustes has no reflection.

A row whose reference was not reached draws the two panels it has and says `not run` in the
caption rather than drawing an empty third panel that would read as "nothing differed".
"""

import os

from sc_fixture import as_points, read_f64
from sc_svg import contact_sheet, finite, write

#: The two fixtures drawn, by name. Fixed, so the PNG set is the same shape on every run.
SHAPE_FIXTURES = ("lesmis", "tree-balanced")


def draw(directory, fixtures, rows):
    """Every name's SVG on every shape fixture, plus the contact sheet. Returns what it wrote."""
    shapes = os.path.join(directory, "shapes")
    os.makedirs(shapes, exist_ok=True)
    starts = offsets(directory, fixtures)
    written = []
    for fixture in fixtures:
        if fixture.name not in SHAPE_FIXTURES:
            continue
        for row in rows:
            path = os.path.join(shapes, "%s-%s.svg" % (row["name"], fixture.name))
            if _draw_one(directory, fixture, row, path, starts[fixture.name]):
                written.append(os.path.basename(path))
    # One contact sheet per shape fixture, so "all 32 at a glance" is true of each of them
    # rather than of the one that happened to be drawn first. A name whose panel was never
    # written is passed as `None` and gets a card saying so, not a broken-image icon.
    for index, fixture_name in enumerate(SHAPE_FIXTURES):
        entries = []
        for row in rows:
            svg = "%s-%s.svg" % (row["name"], fixture_name)
            entries.append((row["name"], svg if os.path.exists(os.path.join(shapes, svg)) else None))
        name = "sheet.html" if index == 0 else "sheet-%s.html" % fixture_name
        written.append(os.path.basename(
            contact_sheet(os.path.join(shapes, name), entries, fixture_name),
        ))
    print("  shapes: %d SVGs plus %d contact sheets in %s"
          % (len(written) - len(SHAPE_FIXTURES), len(SHAPE_FIXTURES), shapes))
    return written


def _draw_one(directory, fixture, row, path, offset):
    """One SVG, or `False` when there was nothing to draw — and the reason is on the row."""
    name = row["name"]
    theirs = _read(directory, "ref", name)
    ours = _read(directory, "motor", name)
    if theirs is None:
        print("  no shape for %s on %s: not run: no reference coordinates"
              % (name, fixture.name))
        return False
    if ours is None or not ours:
        print("  no shape for %s on %s: not run: no motor coordinates"
              % (name, fixture.name))
        return False
    if len(theirs) < 3 * fixture.n or len(ours) < 3 * fixture.n:
        print("  no shape for %s on %s: not run: the files are shorter than the fixture"
              % (name, fixture.name))
        return False
    reference = as_points(theirs[offset:offset + 3 * fixture.n])
    motor = as_points(ours[offset:offset + 3 * fixture.n])
    if not finite(reference) or not finite(motor):
        print("  no shape for %s on %s: not run: a coordinate is not finite"
              % (name, fixture.name))
        return False
    write(path, name, fixture.name, reference, motor, fixture.edges())
    return True


def _read(directory, kind, name):
    """One arm's coordinates for one name, or `None` when the file is not there."""
    path = os.path.join(directory, kind, "%s.f64" % name)
    if not os.path.exists(path):
        return None
    return read_f64(path)


def offsets(directory, fixtures):
    """Where each fixture's coordinates start, over the whole set, in the file's own order.

    The files carry no header, so the offset is the sum of the node counts of the fixtures
    before it — read from `conformance.jsonl`, which is the order the emit wrote them in and
    the order every arm walks them in. Keyed by name so a caller cannot pair an offset with the
    wrong fixture.
    """
    found, offset = {}, 0
    for fixture in fixtures:
        found[fixture.name] = offset
        offset += 3 * fixture.n
    return found
