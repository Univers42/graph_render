/**
 * Which force session a request finds: the engine the last run asked for, on the arm the toggle
 * chose. Split from `session.ts`, which owns the runs; this owns the one call that makes a
 * session, so the warm start, the restart and the GPU arm cannot disagree on it.
 */
import type { Built } from "./built.ts";
import { gpuPort } from "./gpuPort.ts";
import type { ForcePort, ForceSeed, Growable } from "./live.ts";
import type { MotorLike, SessionDeps } from "./session.ts";

/**
 * A session over `built`, or `null` on a motor with none. With the GPU arm on it is a particle
 * mesh whatever the run asked for — the device ticks no other engine — wrapped so its ticks
 * reach the device (`gpuPort.ts`).
 */
export function openForce<Handle>(
  motor: MotorLike<Handle>, built: Built<Handle>, deps: SessionDeps<Handle>, seed?: ForceSeed,
): (ForcePort & Growable<Handle>) | null {
  const gpu = deps.gpu?.() === true;
  const session = motor.forceSession?.(built.handle, undefined, gpu ? "particle_mesh" : built.engine, seed) ?? null;
  return gpu && session !== null ? gpuPort(session) : session;
}

/**
 * The session a force request finds: seeded at the picture the last run drew and born cold,
 * so its first frame repaints that picture instead of replacing it, and a drag or a knob wakes
 * it from there. A scatter (`settle.ts`) is no picture to keep: that session starts hot from
 * the motor's spiral and settles on screen.
 *
 * Measured before this (2026-10-03): every force layout was replaced on the first frame by one
 * settle from the spiral, so ForceAtlas2 and DrL drew identical bounds.
 */
export function startSession<Handle>(
  motor: MotorLike<Handle>, built: Built<Handle>, deps: SessionDeps<Handle>,
): (ForcePort & Growable<Handle>) | null {
  if (!built.warm) return openForce(motor, built, deps);
  const session = openForce(motor, built, deps, "layout");
  session?.reheat(0);
  return session;
}
