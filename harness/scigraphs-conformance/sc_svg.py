"""One SVG per name per fixture: SciGraphs left, the motor right, and the motor
Procrustes-aligned **over** SciGraphs in a second colour.

**SVG, not a raster format, because the job added no dependency and SVG is text.** One file per
(name, fixture) with three panels side by side. The first two are each fitted to their own
bounding box, so a drawing whose scale is off by a factor of a thousand still reads as a shape
rather than as a dot; the third is fitted to the **reference's** box and draws both, which is
what makes it the panel that decides anything: the Procrustes fit has removed the translation,
the uniform scale and the rotation, so what is left over is the shape.

The overlay cannot see a **reflection**, because Procrustes has no reflection. That is why the
metrics report the fit twice (`sc_metrics.disparity`) and why the panel is worth reading by
eye: a mirrored drawing lands on the reference here as a mirror image of it, which is visible
and is not a failure of the fit.

Three panels, 320x320 each, in one 980x436 file with a title and a per-panel caption.
"""

import math
import os

from sc_metrics import apply_similarity

#: Panel geometry. Square, so a rotation is visible as a rotation and not as a shear.
PANEL = 320
GAP = 20
MARGIN = 40
HEADER = 40
FOOTER = 56

#: The colours, named by what they are rather than by what they look like.
REFERENCE = "#1f4fd8"
MOTOR = "#d81f4f"
OVERLAY = "#0f9d58"
GHOST = "#9aa1ad"


def placement(points, size=PANEL, margin=18):
    """A `(point) -> (x, y)` closure that fits `points` into one panel.

    A degenerate extent — every point in a line, or all on one spot — gets `1.0` for that
    axis's scale rather than a division by zero: a graph with no edges is a shape too. And the
    smaller of the two scales is used, so the aspect ratio is kept and a circle stays a circle.
    """
    xs = [p[0] for p in points]
    ys = [p[1] for p in points]
    span_x = max(xs) - min(xs)
    span_y = max(ys) - min(ys)
    scale = min(
        (size - 2 * margin) / span_x if span_x > 0 else float("inf"),
        (size - 2 * margin) / span_y if span_y > 0 else float("inf"),
    )
    if not math.isfinite(scale):
        scale = 1.0
    centre_x = (max(xs) + min(xs)) / 2
    centre_y = (max(ys) + min(ys)) / 2

    def place(point):
        return (
            size / 2 + (point[0] - centre_x) * scale,
            size / 2 - (point[1] - centre_y) * scale,
        )

    return place


def _lines(edges, points, place, colour, width):
    """Every edge as one `<line>`.

    The panel is a `<g transform=…>`, so a layer needs no offset of its own: everything here is
    in panel-local coordinates and the group's translate does the rest. The edges are the
    graph's and never change; only the layer's points do, which is why `points` is a parameter
    rather than a global.
    """
    if not edges or width <= 0:
        return []
    drawn = []
    for source, target in edges:
        x1, y1 = place(points[source])
        x2, y2 = place(points[target])
        drawn.append('    <line x1="%.2f" y1="%.2f" x2="%.2f" y2="%.2f" />' % (x1, y1, x2, y2))
    return drawn


def _dots(points, place, radius):
    """One `<circle>` per node, through `place`."""
    return [
        '    <circle cx="%.2f" cy="%.2f" r="%.2f" />'
        % (place(point)[0], place(point)[1], radius)
        for point in points
    ]


def _panel(offset_x, title, layers, place, edges, edge_width):
    """One panel: a white card, then each `(points, colour, node radius)` layer over it.

    The layers are drawn in order, so the overlay panel is the reference in grey with the
    aligned motor in green on top of it — the motor lands on the reference where they agree
    and stands visibly apart where they do not.
    """
    # Clipped to the card: an overlay that does not agree runs off the panel, and an edge that
    # leaves the drawing is a line across the whole page. Both make the panel unreadable, and
    # unreadable panels are how a shape difference gets called "looks the same".
    clip = "clip-%d" % int(offset_x)
    body = [
        '  <clipPath id="%s"><rect x="0" y="0" width="%d" height="%d" /></clipPath>'
        % (clip, PANEL, PANEL),
        '  <g transform="translate(%.1f,%.1f)">' % (offset_x, MARGIN + HEADER),
        '    <rect x="0" y="0" width="%d" height="%d" fill="#ffffff" stroke="#c8ccd4" />'
        % (PANEL, PANEL),
        '    <g clip-path="url(#%s)">' % clip,
    ]
    for points, colour, radius in layers:
        if edge_width > 0:
            body.append('    <g stroke="%s" stroke-width="%.2f" opacity="0.5" fill="none">'
                        % (colour, edge_width))
            body.extend(_lines(edges, points, place, colour, edge_width))
            body.append("    </g>")
        body.append('    <g fill="%s">' % colour)
        body.extend(_dots(points, place, radius))
        body.append("    </g>")
    body.append("    </g>")
    body.append("  </g>")
    body.append(
        '  <text x="%.1f" y="%.1f" text-anchor="middle" font-size="13" fill="#3a3f4b">%s</text>'
        % (offset_x + PANEL / 2, MARGIN + HEADER + PANEL + 20, _escape(title))
    )
    return "\n".join(body)


def align(theirs, ours):
    """The motor's points in the reference's frame: the Procrustes similarity transform.

    From [`sc_metrics.apply_similarity`], which builds it out of `scipy.linalg`'s own rotation
    and scale rather than re-deriving the fit — one fit in the pipeline, and it is the one the
    reported disparity comes from.

    `None` when the fit is not defined — no more points than columns cannot fill a rotation —
    and the caller then draws the third panel as the motor on its own, captioned as such,
    rather than pretending a fit happened.
    """
    moved = apply_similarity(theirs, ours)
    if moved is None:
        return None
    return [(float(p[0]), float(p[1])) for p in moved]


def write(path, name, fixture_name, theirs, ours, edges):
    """The three-panel SVG for one name over one fixture."""
    count = min(len(theirs), len(ours))
    reference = [(p[0], p[1]) for p in theirs[:count]]
    motor = [(p[0], p[1]) for p in ours[:count]]
    aligned = align(theirs[:count], ours[:count])
    radius = max(1.2, min(4.0, 320.0 / max(count, 1) ** 0.5))
    width = MARGIN * 2 + PANEL * 3 + GAP * 2
    height = MARGIN * 2 + HEADER + FOOTER + PANEL

    body = [
        '<?xml version="1.0" encoding="UTF-8"?>',
        '<svg xmlns="http://www.w3.org/2000/svg" width="%d" height="%d" '
        'viewBox="0 0 %d %d" font-family="ui-sans-serif,system-ui,sans-serif">'
        % (width, height, width, height),
        '  <rect width="100%%" height="100%%" fill="#f7f8fa" />',
        '  <text x="%d" y="%d" font-size="17" font-weight="600" fill="#1b1f27">%s on %s'
        '</text>' % (MARGIN, MARGIN + 18, _escape(name), _escape(fixture_name)),
        '  <text x="%d" y="%d" font-size="12" fill="#4a5160">SciGraphs left, motor right, '
        'motor Procrustes-aligned over SciGraphs: grey is SciGraphs, green is the motor</text>'
        % (MARGIN, MARGIN + 34),
    ]
    body.append(_panel(
        MARGIN, "SciGraphs reference", [(reference, REFERENCE, radius)],
        placement(reference), edges, 0.6,
    ))
    body.append(_panel(
        MARGIN + PANEL + GAP, "motor, own scale", [(motor, MOTOR, radius)],
        placement(motor), edges, 0.6,
    ))
    layers = [(reference, GHOST, radius * 0.8)]
    if aligned is not None:
        # Both layers are placed by the **reference's** fit: the overlay is the motor in the
        # reference's frame, so the two are directly comparable and a difference in shape is
        # the only thing that can be seen.
        layers.append((aligned, OVERLAY, radius))
        title = "aligned over SciGraphs"
    else:
        title = "no Procrustes fit: %d points, %d columns" % (count, 3)
    body.append(_panel(
        MARGIN + 2 * (PANEL + GAP), title, layers, placement(reference), edges, 0.6,
    ))
    body.append("</svg>")
    os.makedirs(os.path.dirname(path) or ".", exist_ok=True)
    with open(path, "w") as handle:
        handle.write("\n".join(body) + "\n")


def contact_sheet(path, entries, fixture_name, columns=6):
    """One HTML page holding every name's panel, for a single screenshot.

    An HTML page rather than one giant SVG because the sheet has to read as a grid of
    thumbnails: at contact-sheet size the question is "does this one look like the reference",
    not "which node is where".

    **A name with no panel gets a card that says so.** An `<img>` pointing at a file that was
    never written is a broken-image icon in a directory of evidence, which reads as "nothing
    differed"; a card that names the reason reads as what it is.
    """
    cells = []
    for name, svg in entries:
        if svg is None:
            cells.append(
                '<figure class="absent"><figcaption>%s</figcaption>'
                "<p>not run: no motor layout for this name, so there is nothing to draw "
                "beside the reference.</p></figure>" % _escape(name)
            )
            continue
        cells.append(
            '<figure><img src="%s" alt="%s"><figcaption>%s</figcaption></figure>'
            % (_escape(svg), _escape(name), _escape(name))
        )
    html = (
        '<!doctype html><meta charset="utf-8">'
        "<style>body{margin:0;background:#f7f8fa;font-family:ui-sans-serif,system-ui,sans-serif}"
        "h1{font-size:18px;margin:16px 20px 4px}"
        "p{margin:0 20px 12px;font-size:12px;color:#4a5160}"
        ".grid{display:grid;grid-template-columns:repeat(%d,1fr);gap:8px;padding:0 16px 16px}"
        "figure{margin:0;background:#fff;border:1px solid #dfe3ea;padding:4px}"
        "img{width:100%%;display:block}"
        "figcaption{font-size:11px;text-align:center;padding:2px 0;color:#1b1f27}"
        "figure.absent{display:flex;flex-direction:column;justify-content:center;"
        "align-items:center;min-height:120px;background:#fff;border:1px dashed #c8ccd4}"
        "figure.absent p{font-size:11px;text-align:center;padding:0 8px;margin:6px 0 0}</style>"
        "<h1>SciGraphs conformance: all 32 names on %s</h1>"
        "<p>Each cell is that row's own three-panel SVG: SciGraphs left, the motor right, and "
        "the motor Procrustes-aligned over SciGraphs &#8212; grey is SciGraphs, green is the "
        "motor.</p>"
        '<div class="grid">%s</div>'
    ) % (columns, _escape(fixture_name), "".join(cells))
    os.makedirs(os.path.dirname(path) or ".", exist_ok=True)
    with open(path, "w") as handle:
        handle.write(html)
    return path


def _escape(text):
    return str(text).replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")


def finite(points):
    """Whether a point list can be drawn at all: a `NaN` in it would emit `nan` into the SVG."""
    return bool(points) and all(math.isfinite(p[0]) and math.isfinite(p[1]) for p in points)
