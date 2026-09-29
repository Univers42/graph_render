"""Rows: colouring, search, bad query."""
import json
import re
from frbase import BUSY_S, MISSING, Missing, Unjudgeable, guarded, row
from frcolour import bytes_of, bytes_of_colour, colour_of_node, filter_of, normalised, reset, sample, stops_of, visible_from_view
from frgraph import with_tags
from frpage import console, drive, need, settled, state
from frrowsa import add_groups, with_group

def key_of(record, by):
    if by == "kind":
        return record["kind"]
    if by == "db":
        return record["db"]
    # Ponytail: a node with several tags is coloured from its first one here. The studio may
    # pick another of them; on the probe's graph that moves exactly the nodes carrying more
    # than one tag, and the escape hatch is a query against a single-tag node.
    return record["tags"][0] if record["tags"] else None


def row_colour_by_key(studio, records):
    name = "colour-by-key"
    expectation = "colour by kind, tag and db: equal keys share a colour, different keys differ"

    def body():
        need(studio, "colour")
        reset(studio)
        lines, agreed = [], True
        for by in ("kind", "tag", "db"):
            with_tags(records)
            drive(studio, "colour", name=by)
            settled(studio, 5.0)
            if state(studio)["colourBy"] != by:
                raise Unjudgeable(f"`colour {by}` left the appearance at {state(studio)['colourBy']}")
            seen = {record["index"]: colour_of_node(studio, record["index"]) for record in records}
            groups = {}
            for record in records:
                groups.setdefault(key_of(record, by), set()).add(seen[record["index"]])
            split = sum(1 for colours in groups.values() if len(colours) > 1)
            shared = sum(1 for colours in groups.values() if len(colours) == 1)
            agreed = agreed and split == 0 and shared > 1
            lines.append(f"{by}: {len(groups)} keys, {shared} one colour each, {split} split")
        return row(name, expectation, "; ".join(lines), agreed)

    return guarded(name, expectation, body)


def metric_analysis(studio):
    """An analysis that measures rather than labels, taken from the studio's own catalog."""
    analyses = state(studio)["analyses"] or []
    labelled = ("analysis.communities.", "analysis.components.")
    for name in analyses:
        if not name.startswith(labelled):
            return name
    raise Missing("the catalog offers no analysis that measures a metric"
                  + (f" (it offers {', '.join(analyses)})" if analyses else " at all"))


def row_colour_by_metric(studio, records):
    name = "colour-by-metric"
    expectation = "the metric colouring is the look library's own inferno ramp, byte for byte"

    def body():
        need(studio, "analysis", "colour")
        analysis = metric_analysis(studio)
        drive(studio, "analysis", id=analysis)
        settled(studio)
        values = state(studio)["values"]
        if not values:
            raise Unjudgeable(f"`analysis {analysis}` left the studio with no per-node values")
        drive(studio, "colour", name="analysis")
        settled(studio, 5.0)
        stops = stops_of()
        wanted = [bytes_of(MISSING) if value != value else bytes_of(sample(stops, t))
                  for value, t in zip(values, normalised(values))]
        seen = [colour_of_node(studio, i) for i in range(len(values))]
        wrong = [i for i, (want, got) in enumerate(zip(wanted, seen)) if want != got]
        by_value = {}
        for value, colour in zip(values, seen):
            by_value.setdefault(value, set()).add(colour)
        split = sum(1 for colours in by_value.values() if len(colours) > 1)
        return row(name, expectation,
                   f"{len(values)} nodes, {len(wrong)} colours off the ramp, {split} values split",
                   not wrong and split == 0)

    return guarded(name, expectation, body)


def row_groups_beat_metric(studio, records):
    name = "groups-override-metric"
    expectation = "with groups defined, a grouped node keeps its group's colour over the metric"

    def body():
        need(studio, "addgroup", "analysis", "colour")
        reset(studio)
        add_groups(studio)
        analysis = metric_analysis(studio)
        drive(studio, "analysis", id=analysis)
        settled(studio)
        drive(studio, "colour", name="analysis")
        settled(studio, 5.0)
        if state(studio)["colourBy"] != "analysis":
            raise Unjudgeable("the metric colouring is not on, so the row would judge nothing")
        node = with_group(studio, records, kind="note", db="db.beta")
        seen = colour_of_node(studio, node["index"])
        return row(name, expectation, f"node {node['index']} is {seen} under the metric colouring",
                   seen == bytes_of_colour("#ff0000"))

    return guarded(name, expectation, body)


def search_set(records, text):
    return {record["index"] for record in records if text.lower() in record["label"].lower()}


def row_search(studio, records):
    name = "search-highlight"
    expectation = "the search text highlights exactly the labels it matches"

    def body():
        need(studio, "search")
        reset(studio)
        drive(studio, "search", text="Alpha")
        settled(studio, 5.0)
        # Ponytail: the studio lights a search by hiding every node the text does not match, so
        # the view's hidden mask is the highlight. A studio that dimmed instead of hiding would
        # read as "everything matches" here, and the row would fail rather than pass.
        seen = visible_from_view(studio)
        mine = search_set(records, "Alpha")
        return row(name, expectation, f"view {len(seen)}, probe {len(mine)}",
                   seen == mine)

    return guarded(name, expectation, body)


def row_search_fit(studio, records):
    name = "search-fit"
    expectation = "fitresults leaves every match inside the canvas"

    def body():
        need(studio, "search", "fitresults")
        reset(studio)
        drive(studio, "search", text="Alpha")
        drive(studio, "fitresults")
        settled(studio, BUSY_S)
        report = state(studio)
        if report["frame"] is None or report["frame"]["x"] is None:
            raise Missing("the view carries no frame, so no match can be placed on screen")
        frame, camera = report["frame"], report["camera"]
        left, top, width, height = studio.read()["box"]
        inside, outside = 0, []
        for index in sorted(search_set(records, "Alpha")):
            x = frame["x"][index] * camera["scale"] + camera["x"]
            y = frame["y"][index] * camera["scale"] + camera["y"]
            if left <= x <= left + width and top <= y <= top + height:
                inside += 1
            else:
                outside.append((x, y))
        return row(name, expectation, f"{inside} of the matches inside, {len(outside)} outside",
                   not outside)

    return guarded(name, expectation, body)


def row_bad_query(studio, records):
    name = "bad-query"
    expectation = "an unbalanced parenthesis is refused with its column, and the filter stands"

    def body():
        need(studio, "query")
        reset(studio)
        drive(studio, "query", query="kind:note")
        settled(studio, 5.0)
        before = filter_of(studio).get("query")
        answer = console(studio, "query " + json.dumps("(kind:note OR kind:tag"))
        settled(studio, 5.0)
        failure = answer.get("error") or (state(studio)["error"] or {})
        message = failure.get("detail", "")
        after = filter_of(studio).get("query")
        column = re.search(r"\b\d+:", message) is not None
        return row(name, expectation, f"refused with {message!r}, filter still {after!r}",
                   column and after == before)

    return guarded(name, expectation, body)
