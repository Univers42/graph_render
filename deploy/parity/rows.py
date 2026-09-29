"""The parity rows: what each one expects, what was measured, and the verdict.

Three rows gate and two are reported. The gating rows compare the saved screenshot against
values the harness computes from the pinned sources, not against hexes typed in here; the
reported rows are the ones the spec says to report rather than assert: the overlap with
the gallery figure, and how the task statement's two hexes sit against the computed ones.
"""

import png
import spec

VIEWPORT = (1920, 1080)
# A corner of the frame the graph never reaches: the fixture's own pixels span 84..1402 by
# 84..1000, so these four are background whatever the graph does.
CORNERS = ((2, 2), (1917, 2), (2, 1077), (1917, 1077))


def row(name, expectation, measured, verdict, gating=True):
    return {"row": name, "expectation": expectation, "measured": measured,
            "verdict": verdict, "gating": gating}


def ok(passed):
    return "PASS" if passed else "FAIL"


def reported(name, expectation, measured):
    """A row the spec says to report, never to assert: its verdict is a count, not a pass."""
    return {"row": name, "expectation": expectation, "measured": measured,
            "verdict": "REPORTED", "gating": False}


def not_run(detail):
    return {"row": "parity-page", "expectation": "the page reports its own state",
            "measured": detail, "verdict": "NOT-RUN", "gating": True}


def delta(measured, expected):
    """The largest per-channel distance between a pixel and what was expected of it."""
    return None if measured is None else max(abs(a - b) for a, b in zip(measured, expected))


def background_row(image, expected, break_):
    """The background pixel, and the four corners as the evidence around it."""
    want = (expected[0], expected[1], expected[2] + 1) if break_ else expected
    at = image.pixel(*CORNERS[0])
    corners = {f"{x},{y}": image.pixel(x, y) for x, y in CORNERS}
    return row(
        "parity-background",
        f"the canvas rounding of encode{spec.BACKGROUND} is {spec.hex_bytes(expected)}"
        + (" with the blue channel off by one (STUDIO_PARITY_BREAK)" if break_ else ""),
        f"({at[0]}, {at[1]}, {at[2]}) at 2,2; corners {corners}",
        ok(at == want),
    )


def viewport_row(report, image):
    state = report.get("state") or {}
    size = [state.get("width"), state.get("height")]
    return row(
        "parity-viewport",
        "the page and the screenshot are the 1920x1080 fig1 frame",
        f"page {size}, screenshot {image.width}x{image.height}",
        ok(size == list(VIEWPORT) and [image.width, image.height] == list(VIEWPORT)),
    )


def fixture_names(fixture):
    return [fixture["nodes"][at]["label"] for at in fixture["labels"]]


def labels_row(report):
    state = report.get("state") or {}
    drawn = set(state.get("labels") or [])
    want = set(fixture_names(report["fixture"]))
    missing = sorted(want - drawn)
    extra = sorted(drawn - want)
    return row(
        "parity-labels",
        "the drawn label set equals the fixture's own 18 (executor.py:498-507)",
        f"{len(drawn)} drawn, {len(want)} in the fixture, {state.get('drawn', 0)} painted;"
        f" missing {missing or 'none'}; extra {extra or 'none'}",
        ok(not missing and not extra and state.get("drawn") == len(want)),
    )


def zero_nodes(report):
    """The nodes the fixture gives a betweenness of exactly zero, as the page sees them."""
    state = report.get("state") or {}
    return [node for node in state.get("nodes") or [] if node.get("betweenness") == 0]


def covered_by(node, others):
    """The first node of another colour whose disc holds this one's centre, if any.

    The painter draws one fill per palette entry in palette order, so a node of a later
    entry paints over the centre of an earlier one, and a 2D painter has no depth test to
    stop it (SciGraphs' render has one: the nodes are 3D glyphs with a depth buffer). The
    margin of one pixel keeps an antialiased rim from counting as a cover.
    """
    for other in others:
        if other.get("fill") == node.get("fill"):
            continue
        dx, dy = other["x"] - node["x"], other["y"] - node["y"]
        if dx * dx + dy * dy < max(other["r"] - 1, 0) ** 2:
            return other
    return None


def inside(rect, x, y):
    """Whether (x, y) is inside a label box, with a pixel of slack for the sprite's edge.

    Ponytail: the slack is one pixel, which is the widest a box can be reported and still
    be one pixel narrower than the one baked (a measurer that rounds down); a wider one would
    excuse a node the label does not really cover.
    """
    return (rect["x"] - 1 <= x <= rect["x"] + rect["width"] + 1
            and rect["y"] - 1 <= y <= rect["y"] + rect["height"] + 1)


def under_label(node, rects):
    """The label box painted over this node's centre, if any."""
    x, y = round(node["x"]), round(node["y"])
    for rect in rects:
        if inside(rect, x, y):
            return rect
    return None


def fill_row(report, image, expected):
    nodes = zero_nodes(report)
    state = report.get("state") or {}
    every = state.get("nodes") or []
    rects = state.get("labelRects") or []
    sampled, off, covered = 0, [], []
    for node in nodes:
        label = under_label(node, rects)
        over = covered_by(node, every)
        if label is not None or over is not None:
            why = f"under the label {label['label']}" if label is not None else f"under {over.get('label')}"
            covered.append(f"{node.get('label')} ({why})")
            continue
        at = image.pixel(round(node["x"]), round(node["y"]))
        if at is None:
            covered.append(f"{node.get('label')} (off the screenshot)")
            continue
        sampled += 1
        off_by = delta(at, expected)
        if off_by is not None and off_by > spec.TOLERANCE:
            off.append(f"{node.get('label')} ({at[0]}, {at[1]}, {at[2]}, {off_by})")
    declared = sorted({node.get("fill") for node in nodes})
    return row(
        "parity-node-fill",
        f"every zero-betweenness node's centre is LUT({spec.FILL_T:.6f}) = {spec.hex_bytes(expected)}"
        f" within {spec.TOLERANCE} per channel",
        f"{sampled} of {len(nodes)} sampled, {len(off)} off {off or ''};"
        f" the page declares {declared};"
        f" not sampled (something else is painted there): {covered or 'none'}",
        ok(not off and sampled > 0 and len(declared) == 1),
    )


def overlap_row(report):
    """Reported, never asserted: the GEXF behind the figure is in neither tree."""
    state = report.get("state") or {}
    drawn = set(state.get("labels") or [])
    shared = sorted(drawn & set(spec.FIG6))
    return reported(
        "parity-fig6-overlap",
        "the 18 names the gallery figure shows, counted and reported",
        f"{len(shared)} of {len(spec.FIG6)}: {', '.join(shared)}"
        f" | only ours: {', '.join(sorted(drawn - set(spec.FIG6))) or 'none'}"
        f" | only the figure's: {', '.join(sorted(set(spec.FIG6) - drawn)) or 'none'}",
    )


def literals_row(expected, report, image):
    """Reported: the task statement's two hexes against what the sources compute."""
    state = report.get("state") or {}
    at = image.pixel(*CORNERS[0]) or (0, 0, 0)
    fills = sorted({node.get("fill") for node in zero_nodes(report)})
    declared = fills[0] if len(fills) == 1 else str(fills)
    return reported(
        "parity-spec-literal",
        f"the statement's {spec.SPEC_BACKGROUND} and {spec.SPEC_FILL}, reported not asserted",
        f"background measured {spec.hex_bytes(at)} against the statement's {spec.SPEC_BACKGROUND};"
        f" the page's fill is {declared}, the statement's is {spec.SPEC_FILL},"
        f" and the computed LUT({spec.FILL_T:.6f}) is {spec.hex_bytes(expected)}",
    )


def judge(report, expected):
    """The rows for one run, or the single NOT-RUN row when the page never reported."""
    if not report.get("state") or "error" in report["state"]:
        return [not_run(str((report.get("state") or {}).get("error", "no state on the page")))]
    state = report["state"]
    nodes = state.get("nodes") or []
    reach = max([round(n["x"]) for n in nodes] + [VIEWPORT[0]]) + 1
    down = max([round(n["y"]) for n in nodes] + [VIEWPORT[1]]) + 1
    image = png.read(report["png"], limit=(reach, down))
    return [
        viewport_row(report, image),
        background_row(image, expected["background"], report.get("breaks")),
        labels_row(report),
        fill_row(report, image, expected["fill"]),
        overlap_row(report),
        literals_row(expected["fill"], report, image),
    ]


def table(report):
    """The report.txt: the rows, the reference they came from, and the run itself."""
    rows = report["rows"]
    width = max(len(row["row"]) for row in rows)
    lines = [f"studio-parity {report['label']} commit {report['commit']}",
             f"viewport {report['viewport'][0]}x{report['viewport'][1]} screenshot {report['png']}", ""]
    for row in rows:
        gate = "gating" if row["gating"] else "reported"
        lines.append(f"[{row['verdict']:>7}] {row['row']:<{width}}  ({gate})")
        lines.append(f"          expect: {row['expectation']}")
        lines.append(f"          measured: {row['measured']}")
    state = report.get("state") or {}
    lines += ["", f"look: {state.get('look', 'not run')}, background: {state.get('background', '?')}, "
              f"labels painted: {state.get('drawn', '?')}, nodes: {len(state.get('nodes') or [])}"]
    return "\n".join(lines) + "\n"
