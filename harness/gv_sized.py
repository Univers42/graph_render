"""The size-pinned DOT writer, for the one engine that sizes its nodes from labels.

`osage` is the reason this exists and the only engine that needs it today. Graphviz sizes a
node from its *rendered label* unless the size is fixed: 54 points for `n0`..`n9` and 57.942
for `n10`..`n99` at the default `nodesize`, which is a font metric of Graphviz's own text
layout and something graph-core has no engine for. The fixture's `box` column carries one
`[width, height]` pair per node **in inches** — graph-core's own table, so the two arms size
the same box — and `sized_dot` writes them as attributes rather than letting Graphviz guess:

- `fixedsize=true` is what makes `shapes.c` take `bb = (width, height)` verbatim instead of
  `fmax` against the label;
- `label=""` leaves the label nothing to demand;
- `margin=0` leaves the default 0.11 inch of padding nothing to add.

`gv_plain.write_dot` is the unsized writer and is shared by twopi, circo and patchwork, so
it stays exactly as it is: their fixtures carry no `box` column and their DOT is unchanged.
An engine whose fixture has no `box` column is refused here rather than drawn at the default
size, because a silent fallback would compare two different boxes and report the difference
as a layout gap.

It lives in its own module because it is one writer for one engine and
`harness/oracle-graphviz.py` is a driver over three arms: keeping the DOT here leaves that
file about the arms and this about the graph it draws.
"""

import os
import sys

from gv_plain import START_SEED, edges_of, parse_plain, run_engine


def sized_dot(path, record):
    """One DOT graph whose node boxes are **pinned**, written to `path`."""
    boxes = record.get("box")
    if boxes is None:
        sys.exit(f"{record['seed']}: no box column, so the node sizes are not pinned")
    if len(boxes) != record["n"]:
        sys.exit(f"{record['seed']}: {len(boxes)} boxes for {record['n']} nodes")
    lines = ["graph g {"]
    for at, (width, height) in enumerate(boxes):
        lines.append(
            f'  n{at} [fixedsize=true,label="",margin=0,width={width!r},height={height!r}];'
        )
    for source, target in edges_of(record):
        lines.append(f"  n{source} -- n{target};")
    lines.append("}")
    with open(path, "w") as handle:
        handle.write("\n".join(lines) + "\n")


def sized_points(engine, tmp, name, record, start=None):
    """The engine's own node coordinates over a size-pinned DOT graph, in dense index order."""
    dot = os.path.join(tmp, f"{name}.dot")
    sized_dot(dot, record)
    _, nodes = parse_plain(run_engine(engine, dot, start), record["n"])
    return [tuple(nodes[f"n{at}"]) for at in range(record["n"])]


# `START_SEED` is re-exported so `oracle-graphviz.py` keeps importing its seed from one place.
__all__ = ["START_SEED", "sized_dot", "sized_points"]