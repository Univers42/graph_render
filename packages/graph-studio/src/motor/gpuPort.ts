/**
 * The GPU arm: a `ForcePort` over a particle-mesh session whose ticks run on the device through
 * the SDK's `GpuMesh` (`docs/decisions/gpu-g1d.md`). The loop, the drag and the deltas are the
 * CPU arm's; only the tick moves, so the port the loop holds is the same shape on both arms.
 *
 * The mesh is opened by the first tick that runs, not by the session: opening asks the browser
 * for an adapter, and a session the user never wakes should not hold a device. Until it answers,
 * a tick runs nothing and returns the session's alpha, so the frame repeats the drawing. A
 * browser with no adapter answers too: the mesh then ticks on the CPU and its tier says why.
 *
 * Caveat: a GPU batch is asynchronous and the loop's step is not. `tick` starts a batch when
 * none is in flight and returns the alpha the last one left, so a frame drawn while a batch
 * runs repeats the last read-back: the drawing moves at the device's rate, not the loop's, and
 * the loop stops one batch after the one that cooled it. Escape hatch: the toggle off is the
 * CPU arm, ticked inside the frame as before.
 */
import type { ForceParams, ForcePort, ForceTier, Growable } from "./live.ts";

/** What the SDK's `GpuMesh` is to this port, named by the members it calls. */
export interface MeshPort<Handle> {
  readonly tier: string;
  readonly reason: string;
  readonly marks: string;
  readonly alpha: number;
  tick(ticks: number): Promise<unknown>;
  pin(row: number, x: number, y: number): void;
  unpin(row: number): void;
  reheat(alpha: number): void;
  setParams(params: Partial<ForceParams>): void;
  grow(handle: Handle): Promise<void>;
}

/** A session the device can drive: the SDK's `ForceSession.gpuMesh`. */
export interface GpuArmed<Handle> {
  gpuMesh?(): Promise<MeshPort<Handle>>;
}

type ArmedSession<Handle> = ForcePort & Growable<Handle> & GpuArmed<Handle>;

/** The mesh this port ticks through, and what it is waiting on. */
interface Arm<Handle> {
  mesh: MeshPort<Handle> | null;
  opening: boolean;
  /** A batch is in flight: a tick starts none, so the frames never queue a backlog. */
  busy: boolean;
  growing: Promise<void> | null;
  /** Why no mesh will open; the session then ticks on the CPU itself. */
  refused: string | null;
}

export const NO_GPU_ARM = "this motor's force session has no GPU arm";

/** The port, with the two members only the GPU arm has made required. */
export type GpuPort<Handle> = ForcePort & Growable<Handle> & Required<Pick<ForcePort, "tier" | "settled">>;

export function gpuPort<Handle>(session: ArmedSession<Handle>): GpuPort<Handle> {
  const arm: Arm<Handle> = { mesh: null, opening: false, busy: false, growing: null, refused: null };
  const driver = (): MeshPort<Handle> | ArmedSession<Handle> => arm.mesh ?? session;
  return {
    get alpha() {
      return arm.mesh?.alpha ?? session.alpha;
    },
    tick: (ticks) => tickOn(session, arm, ticks),
    pin: (row, x, y) => { driver().pin(row, x, y); },
    unpin: (row) => { driver().unpin(row); },
    reheat: (alpha) => { driver().reheat(alpha); },
    setParams: (params) => { driver().setParams(params); },
    positions: () => session.positions(),
    params: () => session.params(),
    // The SDK's release frees the device under a mesh it was driven by.
    release: () => { session.release(); },
    grow: (handle) => { growOn(session, arm, handle); },
    settled: () => arm.growing,
    tier: () => tierOf(arm),
  };
}

function tickOn<Handle>(session: ArmedSession<Handle>, arm: Arm<Handle>, ticks: number): { readonly alpha: number } {
  const mesh = arm.mesh;
  if (mesh === null) {
    if (ticks > 0) open(session, arm);
    return arm.refused === null ? { alpha: session.alpha } : session.tick(ticks);
  }
  if (ticks > 0 && !arm.busy) {
    arm.busy = true;
    void mesh.tick(ticks).then(
      () => { arm.busy = false; },
      // A batch the mesh could not run even on the CPU: the session is released or refused it.
      (error: unknown) => { arm.busy = false; arm.refused = messageOf(error); },
    );
  }
  return { alpha: mesh.alpha };
}

function open<Handle>(session: ArmedSession<Handle>, arm: Arm<Handle>): void {
  if (arm.opening || arm.refused !== null) return;
  if (session.gpuMesh === undefined) {
    arm.refused = NO_GPU_ARM;
    return;
  }
  arm.opening = true;
  session.gpuMesh().then(
    (mesh) => { arm.mesh = mesh; },
    (error: unknown) => { arm.refused = messageOf(error); },
  );
}

/** Before the mesh opens the session grows itself; after, the mesh grows it past its batch. */
function growOn<Handle>(session: ArmedSession<Handle>, arm: Arm<Handle>, handle: Handle): void {
  if (arm.mesh === null) {
    session.grow?.(handle);
    return;
  }
  const growing = arm.mesh.grow(handle);
  arm.growing = growing;
  const done = (): void => {
    if (arm.growing === growing) arm.growing = null;
  };
  // The delta queue awaits `growing` and answers its failure; this only clears the mark.
  growing.then(done, done);
}

function tierOf<Handle>(arm: Arm<Handle>): ForceTier {
  if (arm.mesh !== null) return { tier: arm.mesh.tier, reason: arm.mesh.reason, marks: arm.mesh.marks };
  if (arm.refused !== null) return { tier: "cpu-no-adapter", reason: arm.refused, marks: "" };
  return { tier: "opening", reason: "", marks: "" };
}

function messageOf(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
