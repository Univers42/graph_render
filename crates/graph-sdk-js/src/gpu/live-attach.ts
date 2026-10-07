/**
 * `live-attach.ts` — how a session becomes a `GpuMesh` (`live.ts`): the device opened, or the
 * reason there is none, and the session's pins dropped once it has answered. Split from
 * `live.ts` for the house line cap.
 */

import type { GpuMeshOptions } from "../types.ts";
import { GpuMesh } from "./live.ts";
import type { DriverOpener, GpuArm, GpuDriver } from "./live-driver.ts";
import { type MeshSession, messageOf } from "./live-session.ts";

/** A GPU mesh and the hook its session calls when it is released under it. */
export interface Attached {
  readonly mesh: GpuMesh;
  abandon(): void;
}

/** Opens the device (or says why there is none) and wraps the session. */
export async function attachGpuMesh(session: MeshSession, options: GpuMeshOptions, opener: DriverOpener): Promise<Attached> {
  const n = session.positions().xs.length;
  const opened = n < 2
    ? { driver: null, reason: `the graph has ${n} node(s), and the GPU arm needs two` }
    : await openDriver(opener, n, options.arm ?? "hardware");
  try {
    // After the open, not before: a pin placed while the device opened is the session's to
    // drop, and the mesh starts from none, as its own pin table does.
    session.unpinAll();
    return wrap(new GpuMesh(session, opened.driver, opened.reason));
  } catch (error) {
    // The session went away while the device opened: the device must not outlive the call.
    opened.driver?.destroy();
    throw error;
  }
}

async function openDriver(opener: DriverOpener, n: number, arm: GpuArm): Promise<{ driver: GpuDriver | null; reason: string }> {
  try {
    return { driver: await opener(n, arm), reason: "" };
  } catch (error) {
    return { driver: null, reason: messageOf(error) };
  }
}

function wrap(mesh: GpuMesh): Attached {
  return { mesh, abandon: () => mesh.abandon() };
}
