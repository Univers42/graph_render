/**
 * Row `host-api-unit`, the supersede promise of verdict 7 re-entered (`docs/contract/host-api.md`):
 * a host that calls `el.loadGraph(B)` from inside the `graph-load` handler of `el.loadGraph(A)`.
 *
 * The ordinary case — two calls made before either settles — is `host-load.test.ts`: there the
 * motor is busy and `cancel()` rejects the first. This one is the hole that made the promise a
 * claim about the motor rather than about the studio. A's `graph-load` is dispatched from inside
 * A's own commit, so at that moment nothing is waiting on the motor: `client.busy()` was false,
 * nothing was cancelled, and A resolved with its own counts while B's call ran beside it.
 */
import assert from "node:assert/strict";
import { test } from "node:test";

import { hostVerbs } from "../src/host/api.ts";
import { createPreviews } from "../src/host/previews.ts";
import { watchHost } from "../src/host/watch.ts";
import type { MotorClient } from "../src/motor/client.ts";
import type { GraphSummary } from "../src/motor/protocol.ts";
import { SCRIPTED_META, desk, scriptedClient } from "./desk.ts";

/** Three nodes each, told apart by their edges: what each call hands back says which is which. */
const A = { nodes: [{ id: "a1" }, { id: "a2" }, { id: "a3" }], edges: [] };
const B = { nodes: [{ id: "b1" }, { id: "b2" }, { id: "b3" }], edges: [{ source: "b1", target: "b2" }] };

/** What one host event carried, read without asserting anything about its shape. */
interface Detail {
  readonly edges: number;
  readonly error?: string;
}

function detailOf(event: Event): Detail | null {
  if (!(event instanceof CustomEvent)) return null;
  const value: unknown = event.detail;
  if (typeof value !== "object" || value === null || !("edges" in value) || typeof value.edges !== "number") return null;
  const said: unknown = "error" in value ? value.error : undefined;
  return { edges: value.edges, ...(typeof said === "string" ? { error: said } : {}) };
}

/**
 * A motor that counts the document it was given and answers with a fresh `meta` each run, as the
 * worker's decoder does. What this row measures is which call owns the frame, not what is drawn
 * in it: the drawing is the scripted one either way.
 */
function counting(): MotorClient {
  const scripted = scriptedClient();
  const counted = (text: string): GraphSummary => {
    const doc: unknown = JSON.parse(text);
    if (typeof doc !== "object" || doc === null) throw new Error("the counting motor reads documents only");
    const nodes = "nodes" in doc && Array.isArray(doc.nodes) ? doc.nodes.length : 0;
    const edges = "edges" in doc && Array.isArray(doc.edges) ? doc.edges.length : 0;
    return { name: "counted", nodeCount: nodes, edgeCount: edges, notes: [], buildMs: 1 };
  };
  return {
    ...scripted,
    load: (source) => Promise.resolve(source.kind === "document" ? counted(source.text) : scripted.load(source)),
    layout: async (layout, post) => ({ ...(await scripted.layout(layout, post)), meta: { ...SCRIPTED_META } }),
  };
}

const settled = (): Promise<void> => new Promise((done) => setImmediate(done));

const nameOf = (error: unknown): string => (error instanceof Error ? error.name : "?");

interface Raced {
  /** A's outcome: what it resolved with, or the name it rejected with. */
  readonly first: Promise<string>;
  /** B's outcome: the call the `graph-load` handler made, once it has been made. */
  readonly second: () => Promise<unknown>;
  /** The edges every `graph-load` carried, in the order `document` would hear them. */
  readonly loads: number[];
  readonly errors: string[];
}

/** A studio whose `graph-load` handler re-enters `loadGraph` with B while A is still running. */
function reentering(): Raced {
  const made = desk(counting());
  const host = new EventTarget();
  const verbs = hostVerbs(host, made.studio, Promise.resolve());
  const loads: number[] = [];
  const errors: string[] = [];
  let second: Promise<unknown> | null = null;
  for (const name of ["graph-load", "graph-error"]) {
    host.addEventListener(name, (event) => {
      const detail = detailOf(event);
      if (detail === null) return;
      if (name === "graph-load") {
        loads.push(detail.edges);
        second ??= verbs.loadGraph(B);
      } else {
        errors.push(detail.error ?? "?");
      }
    });
  }
  watchHost({ host, store: made.studio.store, view: { on: () => () => undefined }, previews: createPreviews({ resolver: () => null }) });
  const first = verbs.loadGraph(A).then(() => "resolved", nameOf);
  return { first, second: () => second ?? Promise.resolve("never re-entered"), loads, errors };
}

test("a loadGraph made from inside the graph-load handler supersedes the one that announced", async () => {
  const subject = reentering();
  assert.equal(await subject.first, "CancelledError", "the re-entering call keeps the frame");
  assert.deepEqual(await subject.second(), { nodes: 3, edges: 1, notes: [] }, "B resolves with B's own counts");
  await settled();
  assert.deepEqual(subject.loads, [0, 1], "one graph-load per graph that reached a frame, and B's exactly once");
  assert.deepEqual(subject.errors, [], "a superseded call is not a failure the host is told about");
});