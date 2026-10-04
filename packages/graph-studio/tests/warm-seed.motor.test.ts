// The live session a small graph's layout run leaves, over the real motor: it starts on the
// picture the run drew, and with the knobs the last session held. Each force layout is the
// other's negative control: a session seeded from the spiral draws the same nodes for both.
import assert from "node:assert/strict";
import { test } from "node:test";

import { assembleColumns, createMotor, type ForceSession, type Handle } from "../../../crates/graph-sdk-js/src/index.ts";
import { decodeSnapshot } from "../../graph-render/src/snapshot/decode.ts";
import { DEFAULT_KNOBS, type ForceKnobs } from "../src/motor/live.ts";
import { type Session, createSession } from "../src/motor/session.ts";
import { FIXTURES_URL, SKIP, WASM } from "./motor.ts";

const FA2 = "layout.forceatlas2.barnes_hut";
const DRL = "layout.force.drl";
const KNOBS: ForceKnobs = { ...DEFAULT_KNOBS, gravity: 0.25, charge: -300, linkDistance: 120 };

/** The real session over the real motor, keeping every force session it is handed. */
async function opened(made: ForceSession[]): Promise<Session> {
  const session = createSession({
    motorFrom: async () => {
      const motor = await createMotor(WASM ?? new Uint8Array(0));
      const forceSession = motor.forceSession.bind(motor);
      motor.forceSession = (handle: Handle, params, engine, seed) => {
        const port = forceSession(handle, params, engine, seed);
        made.push(port);
        return port;
      };
      return motor;
    },
    fetchText: () => Promise.reject(new Error("this test fetches nothing")),
    digest: () => Promise.resolve(null),
    assemble: assembleColumns,
    now: () => 0,
  });
  await session.open("unused");
  await session.load({ kind: "synthetic", seed: 3, nodes: 60, degree: 2, shape: "vault" }, FIXTURES_URL);
  return session;
}

/** The live session's drawing as the loop leaves it: `step(0)` reads its alpha and runs no tick. */
function settled(session: Session): Float32Array {
  const port = session.forces();
  if (port === null) throw new Error("a force run left no live session");
  assert.equal(port.step(0), 0, "a small graph's session is born cold");
  const { xs, ys } = port.positions();
  return Float32Array.from([...xs, ...ys]);
}

test("a force run's live session starts on the run's own picture", { skip: SKIP }, async () => {
  const session = await opened([]);
  const fa2 = decodeSnapshot((await session.layout(FA2, null)).bytes);
  const fromFa2 = settled(session);
  const drl = decodeSnapshot((await session.layout(DRL, null)).bytes);
  const fromDrl = settled(session);
  assert.deepEqual(fromFa2, Float32Array.from([...fa2.x, ...fa2.y]), "the session starts on the drawn coordinates");
  assert.deepEqual(fromDrl, Float32Array.from([...drl.x, ...drl.y]));
  assert.notDeepEqual(fromFa2, fromDrl, "two force layouts draw two pictures");
});

test("a re-layout keeps the knobs the last session held", { skip: SKIP }, async () => {
  const made: ForceSession[] = [];
  const session = await opened(made);
  await session.layout(FA2, null);
  session.forces()?.setParams(KNOBS);
  await session.layout(DRL, null);
  assert.deepEqual(session.forces()?.knobs?.(), KNOBS);
  const motorSide = made.at(-1)?.params();
  assert.deepEqual([motorSide?.gravity, motorSide?.charge, motorSide?.link_distance], [0.25, -300, 120]);
});
