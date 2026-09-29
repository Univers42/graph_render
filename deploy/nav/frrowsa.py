"""Rows: queries, filters, relayout, groups."""
import json
from frbase import BUSY_S, Missing, Unjudgeable, guarded, row
from frcolour import bytes_of_colour, colour_of_node, filter_of, layout_calls, reset, visible_from_view
from frpage import console, dispatch, drive, legend, need, settled
from frquery import visible_set

# ------------------------------------------------------------------------------- the rows

QUERY_ROWS = [
    "(kind:note OR kind:tag) AND NOT degree:=0",
    'db:db.alpha AND (path:notes/alpha-three OR tag:#draft) AND "Alpha"',
]

GROUPS = [
    ("red", "kind:note", "#ff0000"),
    ("green", "kind:tag OR kind:note", "#00ff00"),
    ("blue", "db:db.beta", "#0000ff"),
]


def add_groups(studio):
    for name, query, colour in GROUPS:
        drive(studio, "addgroup", name=name, query=query, colour=colour)


def with_group(studio, records, **wanted):
    """The first node matching every one of `wanted`, or nothing to judge."""
    found = [record for record in records if all(record[key] == value for key, value in wanted.items())]
    if not found:
        raise Missing(f"the probe's graph has no node with {wanted}")
    return found[0]


def row_query(studio, records, broken):
    name = "query-visible-set"
    expectation = "a query with AND, OR, NOT and parentheses hides exactly the probe's own set"

    def body():
        need(studio, "query")
        reset(studio)
        lines, agreed = [], True
        for query in QUERY_ROWS:
            console(studio, "query " + json.dumps(query))
            settled(studio, 5.0)
            seen = visible_from_view(studio)
            mine = visible_set(query, records, broken)
            agreed = agreed and seen == mine
            lines.append(f"`{query}`: view {len(seen)}, probe {len(mine)}")
        return row(name, expectation, "; ".join(lines), agreed)

    return guarded(name, expectation, body)


def row_orphans(studio, records):
    name = "filter-orphans"
    expectation = "with orphans on, the hidden set is exactly the degree-0 nodes"

    def body():
        need(studio, "orphans")
        reset(studio)
        answer = dispatch(studio, "orphans", hide=True)
        if answer.get("error") is not None:
            raise Missing(f"`orphans` could not be driven: {answer['error']['detail']}")
        settled(studio, 5.0)
        if not filter_of(studio).get("orphans"):
            raise Unjudgeable("`orphans hide=true` did not turn the orphans filter on")
        seen = visible_from_view(studio)
        mine = {record["index"] for record in records if record["degree"] > 0}
        zeros = [record["index"] for record in records if record["degree"] == 0]
        return row(name, expectation,
                   f"{len(zeros)} degree-0 nodes, {len(seen)} visible, {len(mine)} expected",
                   seen == mine)

    return guarded(name, expectation, body)


def row_kind(studio, records):
    name = "filter-kind"
    expectation = "each kind, toggled off and on, hides exactly the nodes of that kind"

    def body():
        need(studio, "kind")
        lines, agreed = [], True
        everything = {record["index"] for record in records}
        for kind in sorted({record["kind"] for record in records}):
            reset(studio)
            dispatch(studio, "kind", name=kind, hide=True)
            settled(studio, 5.0)
            if kind not in (filter_of(studio).get("hiddenKinds") or []):
                raise Unjudgeable(f"`kind hide=true` did not hide `{kind}`")
            shown = visible_from_view(studio)
            mine = {record["index"] for record in records if record["kind"] != kind}
            dispatch(studio, "kind", name=kind, hide=False)
            settled(studio, 5.0)
            if kind in (filter_of(studio).get("hiddenKinds") or []):
                raise Unjudgeable(f"`kind hide=true` did not bring `{kind}` back")
            back = visible_from_view(studio)
            # Off hides exactly that kind; on leaves nothing hidden at all.
            agreed = agreed and shown == mine and back == everything
            lines.append(f"{kind}: {len(mine)}/{len(records)} kept, {len(back)} back")
        return row(name, expectation, "; ".join(lines), agreed)

    return guarded(name, expectation, body)


def row_no_relayout(studio, records):
    name = "filter-no-relayout"
    expectation = "a filter change costs no layout call while relayout is off"

    def body():
        need(studio, "orphans", "kind", "query")
        reset(studio)
        before = layout_calls(studio)
        drive(studio, "orphans", hide=True)
        settled(studio, 5.0)
        drive(studio, "kind", name="note", hide=True)
        settled(studio, 5.0)
        console(studio, "query " + json.dumps(QUERY_ROWS[0]))
        settled(studio, 5.0)
        after = layout_calls(studio)
        return row(name, expectation, f"{before} → {after} motor layout calls", before == after)

    return guarded(name, expectation, body)


def row_relayout(studio, records):
    name = "filter-relayout"
    expectation = "with relayout on, one filter change costs exactly one layout call"

    def body():
        need(studio, "relayout", "orphans")
        reset(studio)
        drive(studio, "relayout", on=True)
        if not filter_of(studio).get("relayout"):
            raise Unjudgeable("`relayout on=true` did not turn relayout on")
        before = layout_calls(studio)
        dispatch(studio, "orphans", hide=True)
        settled(studio, BUSY_S)
        after = layout_calls(studio)
        return row(name, expectation, f"{before} → {after} motor layout calls", after - before == 1)

    return guarded(name, expectation, body)


def row_groups_first(studio, records):
    name = "groups-first-match"
    expectation = "a node matching two groups takes the first group's colour"

    def body():
        need(studio, "addgroup", "colour")
        reset(studio)
        add_groups(studio)
        dispatch(studio, "colour", name="none")
        settled(studio, 5.0)
        node = with_group(studio, records, kind="note", db="db.beta")
        wanted = bytes_of_colour("#ff0000")
        seen = colour_of_node(studio, node["index"])
        return row(name, expectation, f"node {node['index']} ({node['label']}) is {seen}",
                   seen == wanted)

    return guarded(name, expectation, body)


def row_groups_reorder(studio, records):
    name = "groups-reorder"
    expectation = "movegroup hands the node to the group it was swapped with"

    def body():
        need(studio, "addgroup", "movegroup", "colour")
        reset(studio)
        add_groups(studio)
        dispatch(studio, "colour", name="none")
        settled(studio, 5.0)
        node = with_group(studio, records, kind="note", db="db.alpha")
        before = colour_of_node(studio, node["index"])
        drive(studio, "movegroup", name="red", to=1)
        settled(studio, 5.0)
        after = colour_of_node(studio, node["index"])
        wanted = bytes_of_colour("#00ff00")
        return row(name, expectation, f"node {node['index']} went {before} → {after}", after == wanted)

    return guarded(name, expectation, body)


def row_groups_recolour(studio, records):
    name = "groups-recolour"
    expectation = "recolour repaints the group's nodes in the new colour"

    def body():
        need(studio, "addgroup", "recolour", "colour")
        reset(studio)
        add_groups(studio)
        dispatch(studio, "colour", name="none")
        settled(studio, 5.0)
        node = with_group(studio, records, kind="note", db="db.alpha")
        drive(studio, "recolour", name="red", colour="#00ff00")
        settled(studio, 5.0)
        seen = colour_of_node(studio, node["index"])
        return row(name, expectation, f"node {node['index']} is {seen}",
                   seen == bytes_of_colour("#00ff00"))

    return guarded(name, expectation, body)


def row_groups_legend(studio, records):
    name = "groups-legend"
    expectation = "the legend lists the groups in order, each in its own colour"

    def body():
        need(studio, "addgroup")
        reset(studio)
        add_groups(studio)
        settled(studio, 5.0)
        # The colouring rows follow the groups; only the group rows are judged here.
        rows = legend(studio)[:len(GROUPS)]
        labels = [entry["label"] for entry in rows]
        wanted = [name for name, _, _ in GROUPS]
        swatches = [bytes_of_colour(entry["swatch"]) for entry in rows]
        colours = [bytes_of_colour(colour) for _, _, colour in GROUPS]
        return row(name, expectation, f"legend {labels}",
                   labels == wanted and swatches == colours)

    return guarded(name, expectation, body)
