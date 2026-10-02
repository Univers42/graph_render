"""The fixture set both arms read, and the two little-endian coordinate files.

`conformance.jsonl` is written by `graph-cli emit-conformance-fixtures` and is the **only**
description of the graphs either arm gets: the reference arm builds SciGraphs' object from it
and the motor arm's files are indexed by the same node counts in the same order, so the two
sides cannot be handed different graphs.

**Node order is checked, not assumed.** The rule the fixture encodes is that SciGraphs node
`i` is the motor node whose id sorts `i`-th in byte order. `read_fixtures` sorts the ids and
refuses if the order it recovers is not the order the fixture listed — so a fixture that
broke the rule fails loudly here instead of comparing node 3 against node 7 with every
number still plausible.
"""

import json
import os
import struct

#: The name of the fixture file the motor arm writes, and of the manifest beside it.
FIXTURES = "conformance.jsonl"
MANIFEST = "conformance-manifest.json"


class FixtureError(Exception):
    """The fixture file says something this arm cannot honour. Raised, never repaired."""


class Fixture:
    """One graph: its name, its node count in the order both arms use, and its edges."""

    def __init__(self, record):
        self.name = record["name"]
        self.n = int(record["n"])
        self.nodes = list(record["nodes"])
        self.source = [int(e) for e in record["source"]]
        self.target = [int(e) for e in record["target"]]
        self._check_order(record)
        self._check_mapping(record)

    def _check_order(self, record):
        order = sorted(self.nodes, key=lambda name: name.encode("utf-8"))
        if order != self.nodes:
            raise FixtureError(
                "%s: node ids are not in byte order, so 'index i' is ambiguous"
                % self.name
            )
        if len(set(self.nodes)) != self.n:
            raise FixtureError("%s: a node id is listed twice" % self.name)

    def _check_mapping(self, record):
        """`mapping` is the rule stated rather than followed; check it against the order."""
        for entry in record["mapping"]:
            index, name = int(entry[0]), entry[1]
            if not 0 <= index < self.n or self.nodes[index] != name:
                raise FixtureError(
                    "%s: mapping says node %d is %r, the list says %r"
                    % (self.name, index, name, self.nodes[index])
                )

    def edges(self):
        """The edges as the `(source, target)` pairs `gv_plain.write_dot` and SciGraphs want."""
        return list(zip(self.source, self.target))

    def scigraphs_object(self):
        """The plain dict `apply_graph_layout` takes (`common.py:207`).

        `num_nodes` and a comma-separated `nodes_data` describe the node set; the dense
        pairs ride in as `edge_pairs`, which `_build_networkx_graph` prefers over
        `edges_data` (`common.py:252-259`) precisely because they are already indices. That
        is what makes the node-order rule exact rather than a parse.
        """
        return {
            "num_nodes": self.n,
            "nodes_data": ",".join(self.nodes),
        }

    def edge_pairs(self):
        return [list(pair) for pair in self.edges()]


def read_fixtures(directory):
    """Every fixture, in the file's own order. The order is the emit's, not a sort."""
    path = os.path.join(directory, FIXTURES)
    rows = []
    with open(path) as handle:
        for number, line in enumerate(handle, 1):
            line = line.strip()
            if line:
                try:
                    rows.append(Fixture(json.loads(line)))
                except (KeyError, ValueError, TypeError) as bad:
                    raise FixtureError("%s line %d: %s" % (path, number, bad)) from bad
    if not rows:
        raise FixtureError("%s: no fixtures" % path)
    return rows


def read_manifest(directory):
    with open(os.path.join(directory, MANIFEST)) as handle:
        return json.load(handle)


def read_f64(path):
    """A raw little-endian `f64` file as a flat list of floats."""
    with open(path, "rb") as handle:
        raw = handle.read()
    if len(raw) % 8:
        raise FixtureError("%s: %d bytes is not a whole number of f64" % (path, len(raw)))
    return list(struct.unpack("<%dd" % (len(raw) // 8), raw))


def write_f64(path, values):
    """The same format read back out. `struct` in one call, so no partial file."""
    os.makedirs(os.path.dirname(path) or ".", exist_ok=True)
    with open(path, "wb") as handle:
        handle.write(struct.pack("<%dd" % len(values), *values))


def slice_run(values, fixture, offset):
    """One fixture's coordinates out of a concatenated file, and the next offset.

    The files carry no header, so the offset is a function of the fixture set's own order:
    the arm slices by the same `conformance.jsonl` the emit wrote, in the same order.
    """
    end = offset + 3 * fixture.n
    if end > len(values):
        raise FixtureError(
            "the file holds %d coordinates, %s needs %d" % (len(values), fixture.name, end)
        )
    return values[offset:end], end


def as_points(values):
    """A flat coordinate list as the `(n, 3)` the metrics and the SVGs read."""
    if len(values) % 3:
        raise FixtureError("%d coordinates is not a whole number of points" % len(values))
    return [tuple(values[i:i + 3]) for i in range(0, len(values), 3)]
