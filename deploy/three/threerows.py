"""The 3D rows: what each one claims, and how it is judged.

Every row is a claim about the served app, read from the view's own projection. The input
goes in as a real mouse or a real wheel, so nothing passes here that a user's hand could not
do. A row that cannot be driven with CDP is reported NOT-RUN with the reason, and `--break`
freezes the camera, which is the one fault every row below is written against.
"""
import time

from threepage import DRAG_PX, SPACE_LAYOUT
from verdict import row

# A node has to move this far to count as moved: the eye is hundreds of units off the
# drawing, so a drag of a few pixels moves a node by a few, and a float wobble is not a turn.
MOVED_PX = 1.0
# An angle has to move by this much to count as a turn rather than a float wobble.
ANGLE_MOVED = 1e-6
# A camera distance has to move by this much to count as the wheel having done anything.
DISTANCE_MOVED = 1e-6
# The z spread has to clear this to count as a third column rather than a flat drawing.
SPREAD_MOVED = 1e-6


def by_node(points):
    """The projection keyed by dense index, so a turn does not read as every node changing."""
    return {at["node"]: (at["x"], at["y"]) for at in points}


def moved(before, after):
    """The largest screen distance any node travelled between two projections, in px.

    Keyed by node, not zipped by position: the order is the paint order, which is what a
    turn changes, so zipping the two lists would report the whole drawing as having moved
    even when a camera did nothing.
    """
    worst = 0.0
    for node, (x, y) in after.items():
        was = before.get(node)
        if was is None:
            return float("inf")
        worst = max(worst, abs(was[0] - x), abs(was[1] - y))
    return worst


def row_space(studio):
    """The layout ran and the drawing is 3D: the view has an orbit and a projection."""
    found = studio.positions()
    return row("space-dim", f"`{SPACE_LAYOUT}` draws a 3D frame, not a 2D one",
               f"{len(found['points'])} nodes projected, camera {found['orbit']}",
               len(found["points"]) > 0 and found["orbit"] is not None)


def row_z_spread(studio):
    """The nodes are not all at one depth: a z column that is really there has a range."""
    found = studio.positions()
    depths = [at["depth"] for at in found["points"]]
    spread = max(depths) - min(depths) if depths else 0.0
    return row("space-z-column", "the z column is read, so the nodes are not coplanar",
               f"depth from {min(depths):.2f} to {max(depths):.2f}, spread {spread:.2f}",
               spread > SPREAD_MOVED)


def row_turn(studio):
    """The row this whole gate exists for: a real drag turns the drawing."""
    at = studio.background()
    before = by_node(studio.settled_positions()["points"])
    studio.drag(at, DRAG_PX, 0)
    after = by_node(studio.settled_positions()["points"])
    travel = moved(before, after)
    turned = abs(studio.positions()["orbit"]["yaw"]) > ANGLE_MOVED
    return row("space-turn", f"a {DRAG_PX} px drag at {at} turns the camera and moves the nodes",
               f"the furthest node moved {travel:.2f} px, yaw now {studio.positions()['orbit']['yaw']:.3f}",
               travel > MOVED_PX and turned)


def row_turn_back(studio):
    """The same drag the other way turns it back, so the gesture is a turn and not a jump."""
    at = studio.background()
    before = by_node(studio.settled_positions()["points"])
    yaw = studio.positions()["orbit"]["yaw"]
    studio.drag(at, -DRAG_PX, 0)
    after = by_node(studio.settled_positions()["points"])
    travel = moved(before, after)
    now = studio.positions()["orbit"]["yaw"]
    return row("space-turn-back", f"a {-DRAG_PX} px drag turns the other way",
               f"the furthest node moved {travel:.2f} px, yaw {yaw:.3f} → {now:.3f}",
               travel > MOVED_PX and now < yaw)


def row_tip(studio):
    """A vertical drag tips the drawing, which is a different angle and its own row."""
    at = studio.background()
    before = by_node(studio.settled_positions()["points"])
    studio.drag(at, 0, 120)
    after = by_node(studio.settled_positions()["points"])
    travel = moved(before, after)
    pitch = studio.positions()["orbit"]["pitch"]
    return row("space-tip", f"a 120 px drag down at {at} tips the camera",
               f"the furthest node moved {travel:.2f} px, pitch now {pitch:.3f}",
               travel > MOVED_PX and abs(pitch) > ANGLE_MOVED)


def row_wheel(studio):
    """The wheel pulls the camera in, so the nodes spread out and the radii grow."""
    at = studio.background()
    before = by_node(studio.settled_positions()["points"])
    distance = studio.positions()["orbit"]["distance"]
    studio.wheel(at, -240)
    after = by_node(studio.settled_positions()["points"])
    travel = moved(before, after)
    now = studio.positions()["orbit"]["distance"]
    return row("space-wheel", f"a wheel notch at {at} pulls the camera in",
               f"the furthest node moved {travel:.2f} px, distance {distance:.2f} → {now:.2f}",
               travel > MOVED_PX and now < distance - DISTANCE_MOVED)


def row_pan(studio):
    """The right-drag slides the target, so the drawing moves without the camera turning."""
    at = studio.background()
    before = by_node(studio.settled_positions()["points"])
    yaw = studio.positions()["orbit"]["yaw"]
    studio.drag(at, 120, 0, button="right", buttons=2)
    after = by_node(studio.settled_positions()["points"])
    travel = moved(before, after)
    now = studio.positions()["orbit"]["yaw"]
    return row("space-pan", f"a right-drag at {at} slides the drawing, turning nothing",
               f"the furthest node moved {travel:.2f} px, yaw {yaw:.3f} → {now:.3f}",
               travel > MOVED_PX and abs(now - yaw) < 1e-9)


def row_console_reset(studio):
    """The console's `headon` puts the camera back: it is an action a user can reach."""
    # Turned away first, so the reset has something to undo.
    studio.drag(studio.background(), DRAG_PX, 0)
    turned = studio.settled_positions()
    if abs(turned["orbit"]["yaw"]) < ANGLE_MOVED:
        return row("space-console-reset", "`headon` puts a turned camera back",
                   "the drag did not turn the camera, so there was nothing to reset", False,
                   "the turn row's own precondition failed: the drag left the camera alone")
    studio.console("headon")
    time.sleep(0.4)
    after = studio.positions()
    back = abs(after["orbit"]["yaw"]) < 1e-9
    return row("space-console-reset", "`headon` puts a turned camera back",
               f"yaw {turned['orbit']['yaw']:.3f} → {after['orbit']['yaw']:.3f}", back)


def row_badge(studio):
    """The chrome says the drawing is 3D, so the user is not left guessing.

    The badge's own text carries a leading space, because the frame line beside it is written
    into its own element and a margin is not a space to a screen reader. So this compares the
    stripped text: the claim is that the badge says 3D, not that it is the badge's first
    character.
    """
    found = studio.page.evaluate("""
    (() => {
      const host = document.querySelector('graph-studio');
      const badge = host.shadowRoot.querySelector('.gs-badge');
      const run = host.studio.store.get().run;
      return { text: badge === null ? null : badge.textContent, dim: run === null ? null : run.dim };
    })()
    """)
    text = found["text"]
    return row("space-badge", "the HUD carries a 3D badge while a 3D layout is drawn",
               f"badge {text!r}, run dim {found['dim']}",
               text is not None and text.strip() == "3D" and found["dim"] == 1)


def run_rows(studio, shots):
    """The rows, in the order they are driven, with a screenshot between the camera ones.

    `shoot` returns nothing on purpose, so a screenshot can never be mistaken for a row that
    ran; the `None`s it leaves are what keeps the two apart, and they are dropped here.
    """
    if studio.frozen:
        studio.freeze_camera()
    studio.settle()
    driven = [
        # The two rows that claim the drawing is 3D come first and do not touch the camera:
        # under the negative control they must still pass, or the freeze would be hiding a
        # fault the gate cannot tell from its own.
        row_space(studio), row_z_spread(studio),
        studio.shoot(shots, "01-sphere-head-on.png"),
        row_turn(studio),
        studio.shoot(shots, "02-sphere-turned.png"),
        row_turn_back(studio), row_tip(studio),
        studio.shoot(shots, "03-sphere-tipped.png"),
        row_wheel(studio), row_pan(studio),
        # Last: it turns the camera and puts it back, so every row above it was written
        # against a camera in a known place, and it needs a view at rest.
        row_console_reset(studio), row_badge(studio),
        studio.shoot(shots, "04-sphere-reset.png"),
    ]
    return [entry for entry in driven if entry is not None]
