// The 17 public functions, each bound to the arguments shape the fixture uses. Split out of
// harness/oracle-diff.mjs so that file stays under the 300-line house limit; nothing here is
// oracle-diff-specific except the `fail` message prefix.

import { sha256Hex as sha256 } from "./oracle-attest.mjs";
import { groupModel, layoutGroups } from "./oracle-h9.mjs";
import { canonical, edge, hex, node, unhex, wireLegend, wireModel, wirePatch } from "./oracle-wire.mjs";

const PATCH = ["addedNodes", "updatedNodes", "removedNodeIds", "addedEdges", "updatedEdges", "removedEdgeIds"];

function fail(message) {
  process.stderr.write(`oracle-diff: could not run: ${message}\n`);
  process.exit(2);
}

/** Graphs are defined once per seed and read by name; each read gets fresh objects. */
function graphStore(oracle) {
  const graphs = new Map();
  const fresh = (name) => {
    const [nodes, edges] = graphs.get(name) ?? fail(`graph ${name} is not defined`);
    return [nodes.map(node), edges.map(edge)];
  };
  const define = (a) => {
    graphs.set(a.name, [a.nodes, a.edges]);
    return null;
  };
  return { define, fresh, indexed: (name) => oracle.indexModel(...fresh(name)) };
}

/** The functions over ids, strings and single values. */
const valueFunctions = (oracle) => ({
  makeRecordNodeId: (a) => oracle.makeRecordNodeId(a.source, a.databaseId, a.recordId),
  makeNoteNodeId: (a) => oracle.makeNoteNodeId(a.noteId),
  makeTagNodeId: (a) => oracle.makeTagNodeId(a.tagValue),
  makeEdgeId: (a) => oracle.makeEdgeId(a.source, a.target, a.kind, a.label, a.directed),
  parseNodeId: (a) => oracle.parseNodeId(a.nodeId),
  edgeKindFromType: (a) => oracle.edgeKindFromType(a.type ?? undefined),
  hashString: (a) => oracle.hashString(a.value),
  nodesEqual: (a) => oracle.nodesEqual(node(a.a), node(a.b)),
  edgesEqual: (a) => oracle.edgesEqual(edge(a.a), edge(a.b)),
  isEmptyPatch: (a) => oracle.isEmptyPatch(Object.fromEntries(PATCH.map((k, i) => [k, new Array(a.lengths[i]).fill(0)]))),
  emptyModel: () => wireModel(oracle.emptyModel()),
});

/** The functions over whole graphs. */
function modelFunctions(oracle, { define, fresh, indexed }) {
  return {
    graph: define,
    applyDegreeWeights: (a) => {
      const [nodes, edges] = fresh(a.graph);
      oracle.applyDegreeWeights(nodes, edges);
      return nodes.map((n) => hex(n.weight));
    },
    indexModel: (a) => wireModel(indexed(a.graph)),
    diffGraph: (a) => wirePatch(oracle.diffGraph(indexed(a.previous), indexed(a.next))),
    deriveLegend: (a) => wireLegend(oracle.deriveLegend(indexed(a.graph))),
    neighborhood: (a) => [...oracle.neighborhood(indexed(a.graph), a.id, a.depth)],
    neighborhoodEdges: (a) => {
      const hood = oracle.neighborhoodEdges(indexed(a.graph), a.id, a.depth);
      return { nodeIds: [...hood.nodeIds], edgeIds: [...hood.edgeIds] };
    },
    buildSyntheticModel: (a) => {
      const text = canonical(wireModel(oracle.buildSyntheticModel(unhex(a.n))));
      return a.digest ? { sha256: sha256(text) } : new Canonical(text);
    },
    layoutGroups: (a) => layoutGroups(groupModel(oracle, a)),
  };
}

/** A result already in canonical form (the synthetic model, hashed or not). */
class Canonical {
  constructor(text) {
    this.text = text;
  }
}

/**
 * `(fn, args) -> the canonical JSON text to compare with expect.jsonl`, refusing a function
 * name the fixture invented.
 */
export function evaluator(oracle) {
  const run = { ...valueFunctions(oracle), ...modelFunctions(oracle, graphStore(oracle)) };
  return (fn, args) => {
    if (!Object.hasOwn(run, fn)) fail(`unknown function ${fn}`);
    const result = run[fn](args);
    return result instanceof Canonical ? result.text : canonical(result);
  };
}