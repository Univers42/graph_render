// The live loop's test doubles: a fake port, one that dies with its session, and a rig that
// drives the host's frames by hand. Shared by the loop's test files.
import type { LiveForce } from "../src/motor/live.ts";
import { createForceHost } from "../src/motor/liveLoop.ts";
import type { ForceFrame, Result } from "../src/motor/protocol.ts";

export interface Clock { now: number; perStep: number }

export interface Fake extends LiveForce {
  readonly calls: string[];
  alpha: number;
  decay: number;
}

export function fake(decay: number, clock: Clock = { now: 0, perStep: 0 }): Fake {
  const port: Fake = {
    calls: [], alpha: 0, decay,
    pin: (id, x, y) => { port.calls.push(`pin ${id} ${x} ${y}`); },
    unpin: (id) => { port.calls.push(`unpin ${id}`); },
    setParams: () => { port.calls.push("params"); },
    step: (ticks) => {
      port.calls.push(`step ${ticks}`);
      clock.now += clock.perStep;
      port.alpha *= port.decay;
      return port.alpha;
    },
    positions: () => ({ xs: Float64Array.of(1, 2), ys: Float64Array.of(3, 4) }),
    reheat: (alpha) => { port.alpha = alpha; port.calls.push("reheat"); },
    shuffle: () => { port.calls.push("shuffle"); port.alpha = 1; return port.alpha; },
  };
  return port;
}

export interface Mortal extends Fake {
  /** Set when the graph behind the port was replaced: the session it holds is released. */
  dead: boolean;
}

/** The motor throws `InvalidSessionError` from every call on a released session, so this does. */
export function mortal(decay: number): Mortal {
  const base = fake(decay);
  const guard = (call: string): void => {
    if (mortal.dead) throw new Error(`InvalidSessionError: ${call} on a released session`);
  };
  const mortal: Mortal = {
    dead: false,
    ...base,
    pin: (id, x, y) => { guard("pin"); base.pin(id, x, y); },
    unpin: (id) => { guard("unpin"); base.unpin(id); },
    step: (ticks) => { guard("step"); return base.step(ticks); },
    reheat: (alpha) => { guard("reheat"); base.reheat(alpha); },
    positions: () => { guard("positions"); return base.positions(); },
  };
  return mortal;
}

export interface Rig {
  readonly emitted: Result[];
  readonly frames: () => number;
  readonly tick: () => void;
  readonly scheduled: () => number;
  readonly delays: number[];
}

export function lastFrame(emitted: readonly Result[]): ForceFrame {
  const found = emitted.filter((r) => r.type === "force-frame").at(-1);
  if (found?.type !== "force-frame") throw new Error("no frame was emitted");
  return found.frame;
}

export function rig(port: Fake, clock: Clock = { now: 0, perStep: 0 }, periodMs = 16, live: () => LiveForce | null = () => port) {
  const emitted: Result[] = [];
  const delays: number[] = [];
  let next: (() => void) | null = null;
  let scheduled = 0;
  const host = createForceHost(live, {
    schedule: (run, delayMs) => { next = run; scheduled += 1; delays.push(delayMs); return () => { next = null; }; },
    now: () => clock.now,
    emit: (result) => { emitted.push(result); },
    periodMs,
  });
  const out: Rig = {
    emitted,
    delays,
    frames: () => emitted.filter((r) => r.type === "force-frame").length,
    tick: () => { const run = next; next = null; run?.(); },
    scheduled: () => scheduled,
  };
  return { host, out };
}
