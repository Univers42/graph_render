"""The probe graph and its per-node snapshot."""
import json
from frbase import Missing
from frpage import dispatch, settled, state

# ------------------------------------------------------------------------------ the graph

# (id, kind, database, label, group, tags, path)
NODES = [
    ("n01", "record", "db.alpha", "Alpha One", "g1", ["#core"], "papers/alpha-one"),
    ("n02", "record", "db.alpha", "Alpha Two", "g1", ["#core", "#draft"], "papers/alpha-two"),
    ("n03", "note", "db.alpha", "Alpha Three", "g2", ["#draft"], "notes/alpha-three"),
    ("n04", "note", "db.beta", "Beta One", "g2", ["#core"], "notes/beta-one"),
    ("n05", "tag", "db.beta", "#core", "g3", [], "tags/core"),
    ("n06", "tag", "db.beta", "#edge", "g3", [], "tags/edge"),
    ("n07", "database", "db.beta", "Alpha Database", "g3", ["#core"], "databases/alpha"),
    ("n08", "database", "db.beta", "Beta Database", "g3", ["#edge"], "databases/beta"),
    ("n09", "record", "db.alpha", "Gamma One", "g1", ["#core", "#edge"], "papers/gamma-one"),
    ("n10", "record", "db.beta", "Gamma Two", "g2", ["#draft"], "papers/gamma-two"),
    ("n11", "note", "db.alpha", "Delta One", "g2", [], "notes/delta-one"),
    ("n12", "note", "db.beta", "Delta Two", "g2", ["#edge"], "notes/delta-two"),
    ("n13", "record", "db.alpha", "Epsilon One", "g1", ["#draft"], "papers/epsilon-one"),
    ("n14", "record", "db.alpha", "Alpha Four", "g1", ["#core", "#edge", "#draft"], "papers/alpha-four"),
    ("n15", "tag", "db.alpha", "#draft", "g3", [], "tags/draft"),
    ("n16", "note", "db.beta", "Beta Two", "g2", ["#core", "#edge"], "notes/beta-two"),
]

# (id, source, target, kind)
EDGES = [
    ("e01", "n05", "n01", "tag"), ("e02", "n05", "n02", "tag"), ("e03", "n06", "n09", "tag"),
    ("e04", "n07", "n01", "relation"), ("e05", "n07", "n03", "relation"),
    ("e06", "n08", "n04", "relation"), ("e07", "n08", "n10", "relation"),
    ("e08", "n01", "n02", "relation"), ("e09", "n02", "n03", "relation"),
    ("e10", "n03", "n04", "relation"), ("e11", "n09", "n14", "relation"),
    ("e12", "n15", "n14", "tag"), ("e13", "n14", "n16", "note_link"),
    ("e14", "n16", "n04", "note_link"),
]


def document_text():
    """The graph the rows drive: small, and carrying what the bundled fixtures do not: tags,
    databases and paths, three nodes with no link at all, and a note in each database.

    Ponytail: the gate judges the graph it builds, not the repository's `keys.json`, whose
    twelve nodes hold no note in a second database, so the group rows would have nothing to
    judge. Wrong in the direction of never meeting a shape only that fixture has; the ingest
    tests own that one.
    """
    return json.dumps({
        "version": 1,
        "nodes": [
            {"id": node_id, "kind": kind, "database_id": database, "source": "keys",
             "label": label, "group": group, "weight": 0.5, "version": 1,
             "has_note": kind == "note", "icon": None, "tags": [tag.lstrip("#") for tag in tags], "path": path}
            for node_id, kind, database, label, group, tags, path in NODES
        ],
        "edges": [
            {"id": edge_id, "source": source, "target": target, "kind": kind, "label": "",
             "strength": 1.0, "directed": False, "record_id": None, "child_first": False}
            for edge_id, source, target, kind in EDGES
        ],
    })


def load(studio):
    """Open the document and read it back as one record per node. The reason, or the records."""
    answer = dispatch(studio, "document", name="keys.json", text=document_text())
    if answer.get("error") is not None:
        return f"`document` refused the probe's graph: {answer['error']['detail']}"
    settled(studio)
    meta = state(studio)["meta"]
    if meta is None or not meta.get("labels"):
        return "the studio drew nothing from the probe's document"
    return records_of(meta)


# ------------------------------------------------------------------- the snapshot, per node

def _column(meta, values, index, i):
    """One node's value from a column that may be a list per node or names plus an index.

    Ponytail: `tags`, `dbs` and `paths` are spelled here as either a list per node or a list
    of distinct names with a `Uint16` index beside it, because the snapshot is being written
    right now and this probe must not care which. A third shape (a flat array of indices
    with no names beside it) is read as the index it is. Wrong in the direction of reading a
    key as a name; the escape hatch is the query the rows use, which asks for whole names.
    """
    count = meta["nodeCount"]
    if isinstance(values, list) and len(values) == count:
        return values[i]
    if isinstance(index, list) and len(index) == count and isinstance(values, list):
        slot = index[i]
        return values[slot] if isinstance(slot, int) and slot < len(values) else None
    return None


def _tags_of(meta, i):
    value = _column(meta, meta.get("tags"), meta.get("tag"), i)
    if value is None:
        return None
    if isinstance(value, list):
        return [str(tag) for tag in value]
    return [str(value)]


def records_of(meta):
    """One record per node, from the snapshot's own columns, in the snapshot's own order."""
    records = []
    for i in range(meta["nodeCount"]):
        records.append({
            "index": i,
            "id": meta["ids"][i],
            "label": meta["labels"][i],
            "kind": meta["kinds"][i],
            "db": _column(meta, meta.get("dbs"), meta.get("db"), i),
            "path": _column(meta, meta.get("paths"), meta.get("path"), i),
            "tags": _tags_of(meta, i),
            "degree": meta["degree"][i],
        })
    return records


def with_tags(records):
    if any(record["tags"] is None for record in records):
        raise Missing("the snapshot carries no tags column, so no row can judge a tag")
    return records
