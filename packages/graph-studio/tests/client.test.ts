import assert from "node:assert/strict";
import { test } from "node:test";

import { CancelledError, MotorFailure, createClient } from "../src/motor/client.ts";
import type { Envelope, Port, Request, Result } from "../src/motor/protocol.ts";

const CATALOG = { layouts: ["layout.grid"], posts: [], analyses: [] };
const GRAPH = { name: "g", nodeCount: 2, edgeCount: 1, notes: [], buildMs: 1 };
const ASSETS = { wasmUrl: "/graph_wasm.wasm", fixturesUrl: "/fixtures/" };
const SOURCE = { kind: "fixture", path: "dag/chain.json" } as const;

interface FakePort extends Port {
  readonly sent: Envelope<Request>[];
  closed: boolean;
  reply: (seq: number, body: Result) => void;
}

interface Rig {
  readonly ports: FakePort[];
  readonly spawn: () => Port;
  /** Answers every request that has no answer yet, the way a healthy worker would. */
  readonly answer: (port: FakePort) => void;
}

function fakePort(): FakePort {
  let handler: (message: Envelope<Result>) => void = () => undefined;
  const port: FakePort = {
    sent: [],
    closed: false,
    send: (message) => void port.sent.push(message),
    listen: (next) => {
      handler = next;
    },
    close: () => {
      port.closed = true;
    },
    reply: (seq, body) => handler({ seq, body }),
  };
  return port;
}

function resultFor(request: Request): Result {
  if (request.type === "open") return { type: "opened", catalog: CATALOG };
  if (request.type === "load") return { type: "loaded", graph: GRAPH };
  return { type: "failed", error: { title: "RunRefusedError", code: "code 8 (LayoutFailed)", detail: "refused", hint: "" } };
}

function rig(): Rig {
  const ports: FakePort[] = [];
  const answered = new Set<Envelope<Request>>();
  return {
    ports,
    spawn: () => {
      const port = fakePort();
      ports.push(port);
      return port;
    },
    answer: (port) => {
      for (const message of port.sent) {
        if (answered.has(message)) continue;
        answered.add(message);
        port.reply(message.seq, resultFor(message.body));
      }
    },
  };
}

const settle = (): Promise<void> => new Promise((resolve) => setImmediate(resolve));

function first(ports: readonly FakePort[], index = 0): FakePort {
  const port = ports[index];
  assert.ok(port !== undefined, `port ${index} was never spawned`);
  return port;
}

test("the worker is spawned on the first call and opened once", async () => {
  const { ports, spawn, answer } = rig();
  const client = createClient(spawn, ASSETS);
  assert.equal(ports.length, 0);
  const catalog = client.catalog();
  answer(first(ports));
  assert.deepEqual(await catalog, CATALOG);
  const loading = client.load(SOURCE);
  await settle();
  answer(first(ports));
  assert.deepEqual(await loading, GRAPH);
  assert.deepEqual(first(ports).sent.map((message) => message.body.type), ["open", "load"]);
  assert.deepEqual(first(ports).sent[1]?.body, { type: "load", source: SOURCE, fixturesUrl: "/fixtures/" });
  assert.equal(ports.length, 1);
});

test("a refusal from the motor rejects with what the worker described", async () => {
  const { ports, spawn, answer } = rig();
  const client = createClient(spawn, ASSETS);
  const running = client.layout("layout.grid", null);
  answer(first(ports));
  await settle();
  answer(first(ports));
  await assert.rejects(running, (error: unknown) => error instanceof MotorFailure && error.shown.code === "code 8 (LayoutFailed)" && error.name === "RunRefusedError");
  assert.equal(client.busy(), false);
  assert.equal(first(ports).closed, false, "a refusal leaves the worker and its graph in place");
});

test("stopping closes the worker and rejects what was running", async () => {
  const { ports, spawn, answer } = rig();
  const client = createClient(spawn, ASSETS);
  const loading = client.load(SOURCE);
  answer(first(ports));
  await settle();
  answer(first(ports));
  await loading;
  const running = client.layout("layout.grid", null);
  await settle();
  assert.equal(client.busy(), true);
  assert.equal(client.cancel(), true);
  await assert.rejects(running, CancelledError);
  assert.equal(first(ports).closed, true);
  assert.equal(client.busy(), false);
});

test("after a stop the next call gets a new worker, opened and given the graph again", async () => {
  const { ports, spawn, answer } = rig();
  const client = createClient(spawn, ASSETS);
  const loading = client.load(SOURCE);
  answer(first(ports));
  await settle();
  answer(first(ports));
  await loading;
  const stopped = client.layout("layout.grid", null);
  await settle();
  client.cancel();
  await assert.rejects(stopped, CancelledError);
  const refused = assert.rejects(client.analysis("analysis.depth.bfs"), MotorFailure);
  for (let round = 0; round < 3; round += 1) {
    answer(first(ports, 1));
    await settle();
  }
  await refused;
  assert.deepEqual(first(ports, 1).sent.map((message) => message.body.type), ["open", "load", "analysis"]);
});

test("a load that was stopped is not the graph a new worker is given", async () => {
  const { ports, spawn, answer } = rig();
  const client = createClient(spawn, ASSETS);
  const loading = client.load(SOURCE);
  answer(first(ports));
  await settle();
  client.cancel();
  await assert.rejects(loading, CancelledError);
  void client.catalog();
  answer(first(ports, 1));
  await settle();
  assert.deepEqual(first(ports, 1).sent.map((message) => message.body.type), ["open"]);
});

test("stopping when nothing runs does nothing", async () => {
  const { ports, spawn, answer } = rig();
  const client = createClient(spawn, ASSETS);
  const catalog = client.catalog();
  answer(first(ports));
  await catalog;
  assert.equal(client.cancel(), false);
  assert.equal(first(ports).closed, false);
});

test("an answer nobody waits for is dropped", async () => {
  const { ports, spawn, answer } = rig();
  const client = createClient(spawn, ASSETS);
  const catalog = client.catalog();
  answer(first(ports));
  await catalog;
  first(ports).reply(99, { type: "opened", catalog: CATALOG });
  assert.equal(client.busy(), false);
});

test("a closed client refuses every call", async () => {
  const { ports, spawn, answer } = rig();
  const client = createClient(spawn, ASSETS);
  const catalog = client.catalog();
  answer(first(ports));
  await catalog;
  client.close();
  assert.equal(first(ports).closed, true);
  await assert.rejects(client.catalog(), /closed/);
  assert.equal(ports.length, 1);
});

const OTHER = { kind: "fixture", path: "dag/diamond.json" } as const;

function failure(title: string): Result {
  return { type: "failed", error: { title, code: null, detail: title, hint: "" } };
}

async function loadedClient({ ports, spawn, answer }: Rig): Promise<ReturnType<typeof createClient>> {
  const client = createClient(spawn, ASSETS);
  const loading = client.load(SOURCE);
  answer(first(ports));
  await settle();
  answer(first(ports));
  await loading;
  return client;
}

/** Answers the last request sent on `port` with `body`, the way a failing worker would. */
function replyLast(port: FakePort, body: Result): void {
  const asked = port.sent.at(-1);
  assert.ok(asked !== undefined, "nothing was sent");
  port.reply(asked.seq, body);
}

async function serve(rigged: Rig, index: number, rounds = 3): Promise<void> {
  for (let round = 0; round < rounds; round += 1) {
    rigged.answer(first(rigged.ports, index));
    await settle();
  }
}

test("a new source is loaded in a new worker, and the worker that held the old one is closed", async () => {
  const rigged = rig();
  const client = await loadedClient(rigged);
  const pushed: Result[] = [];
  client.onForce?.((result) => void pushed.push(result));
  const next = client.load(OTHER);
  assert.equal(first(rigged.ports).closed, true);
  assert.deepEqual(pushed.map((result) => result.type), ["force-state"]);
  await serve(rigged, 1);
  assert.deepEqual(await next, GRAPH);
  assert.deepEqual(first(rigged.ports, 1).sent.map((message) => message.body), [
    { type: "open", wasmUrl: ASSETS.wasmUrl }, { type: "load", source: OTHER, fixturesUrl: ASSETS.fixturesUrl },
  ]);
});

for (const title of ["MotorTrapError", "RangeError"]) {
  test(`a ${title} retires the worker, and the next one is given the graph again`, async () => {
    const rigged = rig();
    const client = await loadedClient(rigged);
    const running = client.layout("layout.grid", null);
    await settle();
    replyLast(first(rigged.ports), failure(title));
    await assert.rejects(running, (error: unknown) => error instanceof MotorFailure && error.name === title);
    assert.equal(first(rigged.ports).closed, true);
    const again = assert.rejects(client.analysis("analysis.depth.bfs"), MotorFailure);
    await serve(rigged, 1);
    await again;
    assert.deepEqual(first(rigged.ports, 1).sent.map((message) => message.body.type), ["open", "load", "analysis"]);
  });
}

test("a load that fails closes its worker, and the next one is given the graph loaded before", async () => {
  const rigged = rig();
  const client = await loadedClient(rigged);
  const next = client.load(OTHER);
  await serve(rigged, 1, 1);
  replyLast(first(rigged.ports, 1), failure("IngestRefusal"));
  await assert.rejects(next, (error: unknown) => error instanceof MotorFailure && error.name === "IngestRefusal");
  assert.equal(first(rigged.ports, 1).closed, true);
  const again = assert.rejects(client.analysis("analysis.depth.bfs"), MotorFailure);
  await serve(rigged, 2);
  await again;
  assert.deepEqual(first(rigged.ports, 2).sent[1]?.body, { type: "load", source: SOURCE, fixturesUrl: ASSETS.fixturesUrl });
});
