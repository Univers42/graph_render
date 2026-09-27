// The fixture wire format on the TypeScript side — the mirror of
// crates/graph-cli/src/oracle_fixtures/wire.rs, used by harness/oracle-diff.mjs.
//
// Canonical JSON is compact with every object's keys sorted, as serde_json writes a
// `Value`. Every number that is not a count is the 16 hex digits of its IEEE-754 bits,
// so NaN, -0 and ±Infinity survive and no decimal round trip can differ. `null` is how
// both sides write an absent optional; a decoded GraphNode keeps `databaseId`/`group` as
// null (their oracle type is `string | null`) and omits `icon`/`recordId` (optional).

const bits = new DataView(new ArrayBuffer(8));

export function hex(value) {
  bits.setFloat64(0, value);
  return bits.getBigUint64(0).toString(16).padStart(16, "0");
}

export function unhex(text) {
  if (!/^[0-9a-f]{16}$/.test(text)) throw new Error(`bad f64 bits ${JSON.stringify(text)}`);
  bits.setBigUint64(0, BigInt(`0x${text}`));
  return bits.getFloat64(0);
}

function sorted(value) {
  if (Array.isArray(value)) return value.map(sorted);
  if (value === null || typeof value !== "object") return value;
  return Object.fromEntries(
    Object.keys(value)
      .sort()
      .map((key) => [key, sorted(value[key])]),
  );
}

export const canonical = (value) => JSON.stringify(sorted(value));

/** Wire node → GraphNode. */
export function node(w) {
  const n = {
    id: w.id,
    kind: w.kind,
    databaseId: w.databaseId,
    source: w.source,
    label: w.label,
    group: w.group,
    weight: unhex(w.weight),
    version: unhex(w.version),
    hasNote: w.hasNote,
  };
  if (w.icon !== null) n.icon = w.icon;
  return n;
}

/** Wire edge → GraphEdge. */
export function edge(w) {
  const e = {
    id: w.id,
    source: w.source,
    target: w.target,
    kind: w.kind,
    label: w.label,
    strength: unhex(w.strength),
    directed: w.directed,
  };
  if (w.recordId !== null) e.recordId = w.recordId;
  return e;
}

/** GraphNode → wire node. A missing required field stays missing, so it cannot match. */
const wireNode = (n) => ({
  id: n.id,
  kind: n.kind,
  databaseId: n.databaseId ?? null,
  source: n.source,
  label: n.label,
  group: n.group ?? null,
  weight: hex(n.weight),
  version: hex(n.version),
  hasNote: n.hasNote,
  icon: n.icon ?? null,
});

const wireEdge = (e) => ({
  id: e.id,
  source: e.source,
  target: e.target,
  kind: e.kind,
  label: e.label,
  strength: hex(e.strength),
  directed: e.directed,
  recordId: e.recordId ?? null,
});

/**
 * GraphModel → wire model. `nodeById`/`edgeById` are left out because they are `nodes`/
 * `edges` keyed by id in the same order — checked here, since the wire would otherwise
 * be blind to a model where they are not.
 */
export function wireModel(m) {
  const same = (list, byId) => {
    const values = [...byId.values()];
    return list.length === values.length && list.every((x, i) => values[i] === x && byId.get(x.id) === x);
  };
  if (!same(m.nodes, m.nodeById) || !same(m.edges, m.edgeById)) {
    throw new Error("GraphModel whose nodeById/edgeById are not its nodes/edges: the wire format cannot carry it");
  }
  return {
    nodes: m.nodes.map(wireNode),
    edges: m.edges.map(wireEdge),
    adjacency: [...m.adjacency],
    byDatabase: [...m.byDatabase],
    stats: { nodes: m.stats.nodes, edges: m.stats.edges, databases: m.stats.databases, notes: m.stats.notes },
  };
}

export const wirePatch = (p) => ({
  addedNodes: p.addedNodes.map(wireNode),
  updatedNodes: p.updatedNodes.map(wireNode),
  removedNodeIds: p.removedNodeIds,
  addedEdges: p.addedEdges.map(wireEdge),
  updatedEdges: p.updatedEdges.map(wireEdge),
  removedEdgeIds: p.removedEdgeIds,
});

/** Legend → counts; `color` is dropped (H7: colour stays in TypeScript). */
export const wireLegend = (l) => ({
  databases: l.databases.map((d) => [d.id, d.label, d.count]),
  tags: l.tags.map((t) => [t.id, t.label, t.count]),
  kinds: l.kinds.map((k) => [k.kind, k.count]),
});
