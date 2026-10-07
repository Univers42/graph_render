/**
 * `live.ts` — `GpuMesh`: a particle-mesh session ticked on a WebGPU device, with the session
 * kept as the one source of truth (`docs/decisions/gpu-g1d.md`).
 *
 * The device holds the state while it ticks. After every `tick()` batch the positions are read
 * back into the session's own `f64` columns and its alpha is set to the mesh's, so
 * `positions()`, a CPU fallback and a later CPU session all start from what was drawn. The
 * velocities cross back only when the rig is rebuilt (`setParams`, `grow`) and on `release()`:
 * one read-back per batch instead of two.
 *
 * **Ordering.** Every `tick()`, `grow()` and `release()` joins one promise chain, so two never
 * overlap and a release waits for the batch in flight. The verbs (`pin`, `unpin`, `reheat`,
 * `setParams`) apply to the session at once — a refusal is synchronous, as on the CPU — and
 * reach the device at the next tick: a pin as one queue write, a parameter as a rebuild.
 *
 * **Fallback.** No adapter, a software adapter under `arm: "hardware"`, or a graph under two
 * nodes: tier `cpu-no-adapter`, and every tick is the session's own CPU tick. A device lost
 * or a GPU tick that fails: tier `cpu-device-lost`, the positions are the last batch written
 * back, the velocities are zeroed — the momentum of the device's last tick is dropped, not
 * guessed — and the failed batch reruns on the CPU.
 *
 * **What it starts from.** The session's own alpha (which is why `initial_alpha` is not one of
 * the parameters the device reads) and no pin: `gpuMesh()` drops the session's pins, and a pin
 * set through the mesh is the one the device holds. **What it holds.** Nothing on the host
 * between batches: each read-back is one `Float32Array` of `2n` (8 MB at 1M), written into the
 * session's columns and dropped; `release()` frees the device.
 *
 * Caveat: the device ticks in `f32`, so a GPU mesh is held to a measured bound against the CPU
 * mesh and not to its bytes (`docs/measurements/gpu-g1.md`). Caveat: the collide's jiggle is
 * keyed on the mesh's own tick count, which starts at 0, where the CPU keys it on the session's;
 * the two differ only for nodes at the same point.
 */

import { GpuMeshRefusedError } from "../errors.ts";
import { asU32 } from "../force-params.ts";
import type { ForceParams, ForceTick, GpuTier, Handle } from "../types.ts";
import { Refusal } from "./adapter.ts";
import type { GpuDriver } from "./live-driver.ts";
import { type MeshSession, messageOf, stateOf, writeBack } from "./live-session.ts";

/** A particle-mesh session ticked on the GPU. {@link ForceSession.gpuMesh} is the only way to get one. */
export class GpuMesh {
  readonly #session: MeshSession;
  #driver: GpuDriver | null;
  #tier: GpuTier;
  #reason: string;
  readonly #marks: string;
  #alpha: number;
  #law: Readonly<ForceParams>;
  #ticks = 0;
  #loaded = false;
  #stale = true;
  #lost: string | null = null;
  #released = false;
  #chain: Promise<unknown> = Promise.resolve();
  readonly #staged: ((driver: GpuDriver) => void)[] = [];
  readonly #pins = new Map<number, readonly [number, number]>();

  /** @internal — use {@link ForceSession.gpuMesh}. `driver` is `null` on a CPU tier, and `reason` says why. */
  constructor(session: MeshSession, driver: GpuDriver | null, reason: string) {
    this.#session = session;
    this.#driver = driver;
    this.#tier = driver === null ? "cpu-no-adapter" : "gpu";
    this.#reason = reason;
    this.#marks = driver?.marks ?? "";
    this.#alpha = session.alpha();
    this.#law = session.params();
    void driver?.lost.then((why) => {
      this.#lost = why;
    });
  }

  /** `gpu`, or the CPU tier this mesh fell back to. */
  get tier(): GpuTier {
    return this.#tier;
  }

  /** Why the tier is not `gpu`: the adapter refusal or the device's loss, in the browser's words. */
  get reason(): string {
    return this.#reason;
  }

  /** `vendor/architecture` of the adapter, empty on a CPU tier that never had one. */
  get marks(): string {
    return this.#marks;
  }

  /** The mesh's alpha, which the session's is set to after every batch. */
  get alpha(): number {
    return this.#alpha;
  }

  get released(): boolean {
    return this.#released;
  }

  /** Runs `ticks` ticks, after every batch already asked for. Resolves once the positions are
   *  back in the session: {@link GpuMesh.positions} is then the frame to draw. */
  tick(ticks: number): Promise<ForceTick> {
    const count = asU32(ticks, "ticks");
    this.#requireOpen();
    return this.#enqueue(() => this.#run(count));
  }

  /** The session's positions as the last batch left them: zero-copy, as `ForceSession.positions`. */
  positions(): { readonly xs: Float64Array; readonly ys: Float64Array } {
    return this.#session.positions();
  }

  pin(row: number, x: number, y: number): void {
    this.#requireOpen();
    this.#session.pin(row, x, y);
    this.#pins.set(row, [x, y]);
    this.#staged.push((driver) => driver.pin(row, [x, y]));
  }

  /** {@link GpuMesh.pin} under the name a drag uses, as on `ForceSession`. */
  drag(row: number, x: number, y: number): void {
    this.pin(row, x, y);
  }

  unpin(row: number): void {
    this.#requireOpen();
    this.#session.unpin(row);
    this.#pins.delete(row);
    this.#staged.push((driver) => driver.pin(row, null));
  }

  unpinAll(): void {
    this.#requireOpen();
    this.#session.unpinAll();
    this.#pins.clear();
    this.#staged.push((driver) => driver.unpinAll());
  }

  /** Sets alpha outright; the next tick decays from it. Refused, as on the session, outside `0..=1`. */
  reheat(alpha: number): void {
    this.#requireOpen();
    this.#session.reheat(alpha);
    this.#alpha = alpha;
  }

  /** Sets the session's parameters; the device is rebuilt at them before the next tick. */
  setParams(params: Partial<ForceParams>): void {
    this.#requireOpen();
    this.#session.setParams(params);
    this.#stale = true;
  }

  params(): ForceParams {
    return this.#session.params();
  }

  /** Takes the session onto what `Motor.extend` appended, after the batch in flight. The rig is
   *  rebuilt before the next tick: the new edges, every edge's bias (a grow moves degrees), and
   *  the collide grid, which is sized from `n`. */
  grow(handle: Handle): Promise<void> {
    this.#requireOpen();
    return this.#enqueue(async () => {
      if (this.#driver !== null) await this.#sync(this.#driver);
      this.#session.grow(handle);
      // The rig is now a graph the session no longer is: nothing to sync from it, only a rebuild.
      this.#loaded = false;
      this.#stale = true;
    });
  }

  /** Waits for the batch in flight, writes the positions and the velocities back into the
   *  session, frees the device and hands the session back to its CPU tick. Holds no host copy
   *  of the state after it. */
  release(): Promise<void> {
    this.#requireOpen();
    this.#released = true;
    return this.#enqueue(async () => {
      if (this.#driver !== null) await this.#sync(this.#driver);
      this.#driver?.destroy();
      this.#driver = null;
      this.#session.detach();
    });
  }

  /** @internal — the session was released under this mesh: free the device, write nothing. */
  abandon(): void {
    this.#released = true;
    this.#driver?.destroy();
    this.#driver = null;
  }

  #requireOpen(): void {
    if (this.#released) throw new GpuMeshRefusedError("this GPU mesh is released");
  }

  #enqueue<T>(work: () => Promise<T>): Promise<T> {
    const next = this.#chain.then(work);
    this.#chain = next.catch(() => undefined);
    return next;
  }

  async #run(count: number): Promise<ForceTick> {
    if (this.#driver !== null) {
      try {
        return await this.#onDevice(this.#driver, count);
      } catch (error) {
        this.#fallBack(error);
      }
    }
    const done = this.#session.tick(count);
    this.#alpha = done.alpha;
    return done;
  }

  async #onDevice(driver: GpuDriver, count: number): Promise<ForceTick> {
    if (this.#lost !== null) throw new Error(this.#lost);
    if (this.#stale) await this.#load(driver);
    for (let k = 0; k < count; k += 1) {
      this.#staged.splice(0).forEach((op) => op(driver));
      this.#alpha += (0 - this.#alpha) * this.#law.alpha_decay;
      await driver.tick({ tick: this.#ticks, alpha: this.#alpha });
      this.#ticks += 1;
    }
    const { x } = await driver.read(false);
    writeBack(this.#session, { x, v: null }, this.#alpha);
    const settled = this.#alpha < this.#law.alpha_min;
    return { status: settled ? "settled" : "running", alpha: this.#alpha, ticksRun: count };
  }

  /** (Re)builds the rig from the session, after reading the device's state into it. */
  async #load(driver: GpuDriver): Promise<void> {
    if (this.#loaded) await this.#sync(driver);
    this.#staged.length = 0;
    const state = stateOf(this.#session, this.#pins);
    this.#loaded = false;
    await driver.load(state);
    this.#law = state.law;
    this.#loaded = true;
    this.#stale = false;
  }

  /** The device's positions and velocities into the session, or the fallback when it fails. */
  async #sync(driver: GpuDriver): Promise<void> {
    if (!this.#loaded) return;
    try {
      if (this.#lost !== null) throw new Error(this.#lost);
      const { x, v } = await driver.read(true);
      writeBack(this.#session, { x, v }, this.#alpha);
    } catch (error) {
      this.#fallBack(error);
    }
  }

  /**
   * Drops the device. A refusal (a limit a grown graph exceeds) leaves the session as the last
   * sync wrote it; a loss also drops the momentum the session never received.
   */
  #fallBack(error: unknown): void {
    this.#driver?.destroy();
    this.#driver = null;
    const refused = error instanceof Refusal;
    this.#tier = refused ? "cpu-no-adapter" : "cpu-device-lost";
    this.#reason = messageOf(error);
    if (!refused) {
      const { vxs, vys } = this.#session.velocities();
      vxs.fill(0);
      vys.fill(0);
    }
    this.#alpha = this.#session.alpha();
  }
}
