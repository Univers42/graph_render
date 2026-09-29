"""The local-graph rows: the visible set against an independent BFS, the fit, and the way out.

`--break` makes the depth-1 row expect one node the walk does not reach.
"""
from localdrive import walk
from navrows import row

CLUSTERED = "force/clustered.json"
CHAIN = "dag/chain.json"
NEIGHBOURS = {"neighbours": True}
FLAGS = {"incoming": False, "outgoing": False, "neighbours": False}


def busiest(edges):
    """The node with most links, so depth 1, 2 and 5 reach different sets."""
    count = {}
    for source, target in edges:
        for node in (source, target):
            count[node] = count.get(node, 0) + 1
    return max(sorted(count), key=lambda node: count[node])


def row_depth(studio, depth, expect_extra):
    edges = studio.fixture_edges(CLUSTERED)
    root = busiest(edges)
    studio.open_source(CLUSTERED)
    shown = studio.visible_ids(root, depth, FLAGS)
    wanted = walk(edges, root, depth, FLAGS)
    if expect_extra:
        wanted = sorted([*wanted, "no-such-node"])
    return row(f"local-depth-{depth}", f"the visible set is a BFS of depth {depth} from {root} over the edges",
               f"{len(shown)} visible, {len(wanted)} expected", shown == wanted)


def row_direction(studio):
    edges = studio.fixture_edges(CHAIN)
    studio.open_source(CHAIN)
    cases = [("outgoing", {**FLAGS, "outgoing": True}, 1), ("incoming", {**FLAGS, "incoming": True}, 2),
             ("neighbours", {**FLAGS, **NEIGHBOURS}, 1)]
    seen, failed = [], []
    for name, kinds, depth in cases:
        shown = studio.visible_ids("c", depth, kinds)
        seen.append(f"{name}/{depth}={''.join(shown)}")
        if shown != walk(edges, "c", depth, kinds):
            failed.append(name)
    return row("local-direction", "on the directed chain a-b-c-d-e, incoming, outgoing and neighbours reach what a BFS reaches",
               " ".join(seen), not failed)


def row_fit(studio):
    edges = studio.fixture_edges(CLUSTERED)
    studio.open_source(CLUSTERED)
    shown = studio.visible_ids(busiest(edges), 2, FLAGS)
    stats = studio.arrive()
    return row("local-fit", "every node of the local set is drawn, so inside the viewport, and no other",
               f"drawn {stats['drawnNodes']} of {len(shown)} local, {stats['nodes']} in the graph",
               stats["drawnNodes"] == len(shown) < stats["nodes"])


def row_escape(studio):
    studio.focus_page()
    studio.key("Escape")
    stats = studio.arrive()
    return row("local-escape", "Escape returns to the global graph with every node drawn",
               f"drawn {stats['drawnNodes']} of {stats['nodes']}", stats["drawnNodes"] == stats["nodes"])


def run_rows(studio, broken):
    return [row_depth(studio, 1, broken), row_depth(studio, 2, False), row_depth(studio, 5, False),
            row_direction(studio), row_fit(studio), row_escape(studio)]
