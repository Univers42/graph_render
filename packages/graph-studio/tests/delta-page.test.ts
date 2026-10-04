/**
 * The page's half: the verb's argument check, the refusal a host sees before anything is sent,
 * and the frame drawn up to the node count the last structure snapshot described.
 */
import assert from "node:assert/strict";
import { test } from "node:test";

import { APPLY_DELTAS, ActionRefusal, createDeltas, deltaBatch } from "../src/actions/registry.ts";
import { type DeltasView, createDeltasPage } from "../src/motor/deltasPage.ts";
import type { StudioState } from "../src/state/model.ts";

function node(id: string): Record<string, unknown> {
  return {
    id, kind: "record", database_id: null, source: "test", label: id,
    group: null, weight: 1, version: 1, has_note: false, icon: null,
  };
}

function edge(id: string, source: string, target: string): Record<string, unknown> {
  return {
    id, source, target, kind: "relation", label: "", strength: 1,
    directed: false, record_id: null, child_first: false,
  };
}

test("a batch of records is taken member by member, not passed through", () => {
  const batch = deltaBatch({ nodes: [node("a")], edges: [edge("e", "a", "b")] });
  assert.deepEqual(batch.nodes, [node("a")]);
  assert.deepEqual(batch.edges[0], edge("e", "a", "b"));
});

test("anything that is not `{ nodes, edges }` of objects is refused, with the member named", () => {
  for (const raw of [null, 42, "batch", [], { nodes: [] }, { nodes: {}, edges: [] }, { nodes: [1], edges: [] }]) {
    assert.throws(() => deltaBatch(raw), ActionRefusal, `expected a refusal for ${JSON.stringify(raw) ?? "null"}`);
  }
});

test("a member of the wrong type is refused by name, so a host can fix the call", () => {
  const refused = (raw: unknown): string => {
    try {
      deltaBatch(raw);
    } catch (error) {
      return error instanceof Error ? error.message : "";
    }
    return "";
  };
  assert.match(refused({ nodes: [{ ...node("a"), weight: "heavy" }], edges: [] }), /nodes\[0\]\.weight/);
  assert.match(refused({ nodes: [], edges: [{ ...edge("e", "a", "b"), directed: "yes" }] }), /edges\[0\]\.directed/);
  assert.match(refused({ nodes: [{ ...node("a"), group: 7 }], edges: [] }), /nodes\[0\]\.group/);
});

test("the verb refuses before it reads the batch when there is nothing to take it", async () => {
  const deltas = createDeltas(() => "the element is not in a document", () => Promise.resolve(1));
  assert.equal(deltas.id, APPLY_DELTAS);
  await assert.rejects(deltas.apply("not a batch"), (error: unknown) => {
    assert.ok(error instanceof ActionRefusal);
    assert.equal(error.code, "unavailable");
    assert.match(error.message, /the element is not in a document/);
    return true;
  });
});

test("a batch the verb accepted is the batch that is sent, and the count comes back", async () => {
  const sent: unknown[] = [];
  const deltas = createDeltas(() => null, (batch) => {
    sent.push(batch);
    return Promise.resolve(batch.nodes.length);
  });
  assert.equal(await deltas.apply({ nodes: [node("a"), node("b")], edges: [] }), 2);
  assert.deepEqual(sent, [{ nodes: [node("a"), node("b")], edges: [] }]);
});

interface Drawn {
  readonly xs: number[];
  readonly ys: number[];
  readonly frames: number;
}

function view(drawn: Drawn): DeltasView {
  return {
    setFrame: () => undefined,
    setPositions: (xs, ys) => {
      drawn.frames += 1;
      drawn.xs = [...xs];
      drawn.ys = [...ys];
    },
  };
}

function page(nodeCount: number | null): { readonly deltas: ReturnType<typeof createDeltasPage>; readonly drawn: Drawn } {
  const drawn: Drawn = { xs: [], ys: [], frames: 0 };
  const state = (): StudioState | null => nodeCount === null
    ? null
    : ({ meta: { nodeCount, ids: [], labels: [], kinds: [], groups: [], group: new Uint16Array(0), weight: new Float32Array(0), degree: new Uint32Array(0), maxDegree: 0, tags: [] } } as unknown as StudioState);
  return { deltas: createDeltasPage(view(drawn), state, () => undefined), drawn };
}

test("a frame is drawn up to the node count the last snapshot described", () => {
  const { deltas, drawn } = page(3);
  const xs = Float32Array.from([1, 2, 3, 4, 5]);
  const ys = Float32Array.from([6, 7, 8, 9, 10]);
  deltas.frame(xs, ys);
  assert.equal(deltas.drawn(), 3);
  assert.deepEqual(drawn.xs, [1, 2, 3]);
  assert.deepEqual(drawn.ys, [6, 7, 8]);
});

test("with no snapshot yet the whole frame is drawn, as before a delta batch", () => {
  const { deltas, drawn } = page(null);
  deltas.frame(Float32Array.from([1, 2]), Float32Array.from([3, 4]));
  assert.equal(deltas.drawn(), null);
  assert.deepEqual(drawn.xs, [1, 2]);
  assert.deepEqual(drawn.ys, [3, 4]);
});
