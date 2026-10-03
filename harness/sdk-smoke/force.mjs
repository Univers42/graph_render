// The live force session, end to end through the published surface: create a session over a
// graph, tick it, drag one node, and watch a neighbour move. Nothing here reaches past
// `crates/graph-sdk-js/src/index.ts` — a real consumer has the SDK's types and nothing else.
//
// The four nodes are a path (a--b--c--d) so every node but the ends has a neighbour whose
// position the drag can move: a graph with no edges would satisfy every "positions are finite"
// check below and prove nothing about a pin.

import { ForceSessionRefusedError, InvalidSessionError } from "../../crates/graph-sdk-js/src/index.ts";
import { check, refusedWith } from "./lib.mjs";

const EDGE = (id, source, target) => ({
  id,
  source,
  target,
  kind: "relation",
  label: "",
  strength: 0.5,
  directed: false,
  record_id: null,
});

export async function runForceSection(ctx) {
  const { motor } = ctx;
  const node = (id) => ({
    id,
    kind: "record",
    database_id: null,
    source: "s",
    label: id.toUpperCase(),
    group: null,
    weight: 0.5,
    version: 0,
    has_note: false,
    icon: null,
  });
  const ingest = JSON.stringify({
    version: 1,
    nodes: ["a", "b", "c", "d"].map(node),
    edges: [EDGE("e0", "a", "b"), EDGE("e1", "b", "c"), EDGE("e2", "c", "d")],
  });

  // No layout run first: a session is built from the topology, so this works straight after build.
  const handle = motor.build(ingest);
  const session = motor.forceSession(handle, { gravity: 0.1 });

  const cold = session.positions();
  check("a new session has one row per node", cold.xs.length === 4 && cold.ys.length === 4);
  // Every position the session hands back is checked for finiteness before it is compared.
  // `after.xs[1] !== before[1]` is satisfied by `NaN`, so without this the neighbour checks
  // below would pass on a motor returning NaN for b/c/d — and the header's own claim
  // ("a graph with no edges would satisfy every 'positions are finite' check below") was
  // a check that did not exist (M32).
  check(
    "every position a new session reports is finite",
    [cold.xs, cold.ys].every((column) => column.every(Number.isFinite)),
    JSON.stringify(cold),
  );
  check("the partial parameter set is the motor's own, not a copy", session.params().gravity === 0.1);
  check("an omitted parameter keeps the motor's value", session.params().theta !== 0.1);
  check("a new session starts hot", session.alpha === 1.0);

  const warm = session.tick(5);
  check("five ticks ran and cooled it", warm.ticksRun === 5 && warm.alpha < 1.0 && warm.status === "running");
  check("the alpha the tick reports is the session's own", session.alpha === warm.alpha);

  // The drag: pin node 0 far off and reheat, which is the verb behind "the user moved
  // something, run it again". Nothing moves until the next tick, so tick before reading.
  //
  // `before` is copied *after* the ticks above and read back through a fresh view: a tick
  // can grow wasm memory, which detaches the zero-copy view a previous `positions()` handed
  // out, and `Float64Array.from` over a detached view yields `[]` — which would make every
  // "it moved" comparison below vacuously true (m94).
  const before = Float64Array.from(session.positions().xs);
  session.drag(0, 500, -500);
  session.reheat(1.0);
  session.tick(30);
  const after = session.positions();
  check("the dragged node sits exactly where it was put", after.xs[0] === 500 && after.ys[0] === -500);
  check(
    "every position after the drag is finite",
    [after.xs, after.ys].every((column) => column.every(Number.isFinite)),
    JSON.stringify(after),
  );
  check("its neighbour moved", before.length === 4 && after.xs[1] !== before[1]);
  check("the dragged node's neighbours are not all still", after.xs[1] !== 500 && after.xs[2] !== 500);

  // `pin`/`unpin` as a pair. Both `pin` calls below are refusals, so the success path of
  // `unpin` was covered only transitively through `drag`: replace `force.ts`'s `unpin`
  // body with `{}` and every check in this file stayed green (M33).
  session.pin(1, -500, 500);
  session.tick(20);
  const pinned = session.positions();
  check("a pinned node sits exactly where it was pinned", pinned.xs[1] === -500 && pinned.ys[1] === 500, JSON.stringify({ xs: pinned.xs[1], ys: pinned.ys[1] }));
  session.tick(20);
  check("and it stays there through a later tick", session.positions().xs[1] === -500);
  session.unpin(1);
  session.tick(20);
  const unpinned = session.positions();
  check(
    "an unpinned node is free to move again",
    unpinned.xs[1] !== -500 || unpinned.ys[1] !== 500,
    JSON.stringify({ xs: unpinned.xs[1], ys: unpinned.ys[1] }),
  );

  check(
    "a pin past the last row is refused, not silently dropped",
    await refusedWith(ForceSessionRefusedError, () => session.pin(99, 1, 1)),
  );
  check(
    "a non-finite pin coordinate is refused (D9)",
    await refusedWith(ForceSessionRefusedError, () => session.pin(0, Number.NaN, 0)),
  );
  check(
    "an out-of-range parameter is refused rather than clamped",
    await refusedWith(ForceSessionRefusedError, () => session.setParams({ theta: 9 })),
  );
  check("and the refusal left the parameters alone", session.params().theta !== 9);
  check(
    "a parameter the motor would refuse at creation is refused here too",
    await refusedWith(ForceSessionRefusedError, () => motor.forceSession(handle, { charge: 1 })),
  );

  // `refusedWith` discards whatever the call returned, so a negative control that
  // unexpectedly *succeeds* leaks the session it created (m95). The leak is on the failing
  // path only — the check below has already failed — but a second run in the same process
  // would then hold two sessions. Nothing to do here beyond recording it: the refusal is
  // the behaviour under test, and a success is a failure of the arm, not of the SDK.

  const settled = session.tick(2000);
  check("a long run reports the motor's own settled verdict", settled.status === "settled", `status ${settled.status}`);

  session.unpinAll();
  session.release();
  check("a released session says so", session.released);
  check(
    "and a released session refuses further work",
    await refusedWith(InvalidSessionError, () => session.tick(1)),
  );

  // The graph handle outlives the session: releasing one must not disturb the other.
  check("the graph is still usable after the session is released", motor.nodeCount(handle) === 4);
  motor.release(handle);
}