"""The filtering rows, in order. Rows live in frrowsa/frrowsb; the oracles in frquery/frcolour;
the page access in frpage. `--break` reaches only the query evaluator (see frquery)."""
from frbase import row
from frgraph import load

from frrowsa import row_groups_first, row_groups_legend, row_groups_recolour, row_groups_reorder, row_kind, row_no_relayout, row_orphans, row_query, row_relayout
from frrowsb import row_bad_query, row_colour_by_key, row_colour_by_metric, row_groups_beat_metric, row_search, row_search_fit


def run_rows(studio, broken=False):
    loaded = load(studio)
    if isinstance(loaded, str):
        return [row(name, expectation, "nothing was driven", False, loaded)
                for name, expectation in EXPECTATIONS]
    records = loaded
    return [
        row_query(studio, records, broken),
        row_orphans(studio, records),
        row_kind(studio, records),
        row_no_relayout(studio, records),
        row_relayout(studio, records),
        row_groups_first(studio, records),
        row_groups_reorder(studio, records),
        row_groups_recolour(studio, records),
        row_groups_legend(studio, records),
        row_colour_by_key(studio, records),
        row_colour_by_metric(studio, records),
        row_groups_beat_metric(studio, records),
        row_search(studio, records),
        row_search_fit(studio, records),
        row_bad_query(studio, records),
    ]


EXPECTATIONS = {
    "query-visible-set": "a query with AND, OR, NOT and parentheses hides exactly the probe's own set",
    "filter-orphans": "with orphans on, the hidden set is exactly the degree-0 nodes",
    "filter-kind": "each kind, toggled off and on, hides exactly the nodes of that kind",
    "filter-no-relayout": "a filter change costs no layout call while relayout is off",
    "filter-relayout": "with relayout on, one filter change costs exactly one layout call",
    "groups-first-match": "a node matching two groups takes the first group's colour",
    "groups-reorder": "movegroup hands the node to the group it was swapped with",
    "groups-recolour": "recolour repaints the group's nodes in the new colour",
    "groups-legend": "the legend lists the groups in order, each in its own colour",
    "colour-by-key": "colour by kind, tag and db: equal keys share a colour, different keys differ",
    "colour-by-metric": "the metric colouring is the look library's own inferno ramp, byte for byte",
    "groups-override-metric": "with groups defined, a grouped node keeps its group's colour",
    "search-highlight": "the search text highlights exactly the labels it matches",
    "search-fit": "fitresults leaves every match inside the canvas",
    "bad-query": "an unbalanced parenthesis is refused with its column, and the filter stands",
}
