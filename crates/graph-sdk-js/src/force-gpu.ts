// `ForceSession.gpuMesh()`'s body: the refusals at the door, the device opened, and the bridge a
// GPU mesh drives its session through (`gpu/live.ts`'s `MeshSession`). The bridge is the session's
// public face plus the two things only the session can give — its simple graph, and calls past
// the one-driver refusal (`force-calls.ts`) — so a verb the mesh forwards and a verb a host
// calls behind the mesh's back cannot be confused.

import { GpuMeshRefusedError } from "./errors.ts";
import type { Loaded } from "./calls.ts";
import type { ForceSession } from "./force.ts";
import type { SessionCalls } from "./force-calls.ts";
import { simpleEdges } from "./force-handoff.ts";
import type { ForceParams, ForceSessionId, GpuMeshOptions, Handle } from "./types.ts";
import { type Attached, attachGpuMesh } from "./gpu/live-attach.ts";
import type { MeshSession } from "./gpu/live-session.ts";
import { deviceOpener, liveFault } from "./gpu/live-driver.ts";

export type { Attached } from "./gpu/live-attach.ts";
export type { GpuMesh } from "./gpu/live.ts";

/** What only the session can hand over: its calls, its storage, and the engine it ticks. */
export interface Ownership {
  readonly calls: SessionCalls;
  readonly loaded: Loaded;
  readonly id: ForceSessionId;
  readonly mesh: boolean;
}

/** Sessions with a device being opened for them: a second `gpuMesh()` meanwhile is refused. */
const opening = new WeakSet<ForceSession>();

/**
 * Opens a device for `session` (or says why there is none) and hands the session to the mesh.
 * The session is driven only once the mesh exists: while the device opens, its verbs still work.
 */
export async function driveOnGpu(session: ForceSession, own: Ownership, options: GpuMeshOptions): Promise<Attached> {
  own.calls.requireLive();
  if (!own.mesh) throw new GpuMeshRefusedError("gpuMesh() drives a particle_mesh session, and this one ticks barnes_hut");
  if (own.calls.driven || opening.has(session)) throw new GpuMeshRefusedError("a GPU mesh already drives this session");
  opening.add(session);
  try {
    const host = options.host ?? (typeof navigator === "undefined" ? {} : navigator);
    const attached = await attachGpuMesh(bridge(session, own), options, deviceOpener(host, liveFault.name));
    if (!own.calls.live) {
      attached.abandon();
      throw new GpuMeshRefusedError("the session was released while the device was opening");
    }
    own.calls.drive(true);
    return attached;
  } finally {
    opening.delete(session);
  }
}

function bridge(session: ForceSession, own: Ownership): MeshSession {
  const through = <T>(call: () => T): T => own.calls.lift(call);
  return {
    positions: () => session.positions(),
    velocities: () => session.velocities(),
    edges: () => simpleEdges(own.loaded, own.id),
    params: () => session.params(),
    alpha: () => session.alpha,
    reheat: (alpha: number) => { through(() => { session.reheat(alpha); }); },
    pin: (row: number, x: number, y: number) => { through(() => { session.pin(row, x, y); }); },
    unpin: (row: number) => { through(() => { session.unpin(row); }); },
    unpinAll: () => { through(() => { session.unpinAll(); }); },
    setParams: (params: Partial<ForceParams>) => { through(() => { session.setParams(params); }); },
    grow: (handle: Handle) => { through(() => { session.grow(handle); }); },
    tick: (ticks: number) => through(() => session.tick(ticks)),
    detach: () => { own.calls.drive(false); },
  };
}
