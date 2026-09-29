/**
 * The one place the studio talks to the wasm motor.
 *
 * Every `Motor` call happens here, inside a try, and every column is COPIED out
 * before the next call: a `Column` is a window on the motor's own memory that
 * `Motor#build`/`#layout`/`#post`/`#release` invalidate (C7/C10), so a renderer
 * that held one would draw freed memory. `run` and `post` therefore return a
 * `RunReport` — plain arrays — and nothing downstream ever sees a wasm pointer
 * again.
 *
 * The durations are wall-clock measurements of the HOST around those calls
 * (`performance.now`). They are never fed back into the engine, never hashed and
 * never compared: D8 keeps the clock out of the motor, and this is the studio
 * measuring the motor from outside, which is the one place a duration is a fact
 * about the machine rather than about the graph.
 */

import {
  ColumnId,
  GraphMotorError,
  type AnalysisResult,
  type Column,
  type EdgeGeometryKind,
  type Handle,
  type Motor,
  type NodeGeometryKind,
  type PostResult,
} from "../../../crates/graph-sdk-js/src/index.ts";
import { createMotor } from "../../../crates/graph-sdk-js/src/index.ts";
import { type ColumnInput } from "../core/drawList.ts";
import { type RunReport, layoutReport, withPost } from "./report.ts";

export type { RunReport } from "./report.ts";

/** A motor plus the ingest it last built, and the handles it is holding. */
export class MotorSession {
  readonly #motor: Motor;
  #handle: Handle | null = null;

  private constructor(motor: Motor) {
    this.#motor = motor;
  }

  /** Loads the motor. A load failure resolves to a DEGRADED session (the SDK's
   *  documented behaviour) rather than throwing: the page still renders, with the
   *  reason in the banner. */
  static async open(wasmUrl: string): Promise<MotorSession> {
    const motor = await createMotor(new URL(wasmUrl, globalThis.location?.href ?? "http://localhost/"));
    return new MotorSession(motor);
  }

  get available(): boolean {
    return this.#motor.available;
  }

  /** Every layout the module's own registry offers, in registry order — never a
   *  list written down here (C1). Refuses on a degraded motor, and the caller
   *  shows the refusal. */
  layouts(): readonly string[] {
    return this.#motor.layouts();
  }

  /** Every POST capability the module registers, in registry order (C1) — the
   *  panel's picker, filled the same way the layout picker is, so a capability
   *  registered after this file was written appears with no change here. */
  posts(): readonly string[] {
    return this.#motor.posts();
  }

  /** Every analysis the module registers, in registry order (C1). */
  analyses(): readonly string[] {
    return this.#motor.analyses();
  }

  /** Builds `ingestJson` and returns the handle plus the build duration. The
   *  previous handle is released first: one live graph at a time, and the studio
   *  has no reason to hold two. */
  build(ingestJson: string): { handle: Handle; buildMs: number } {
    this.release();
    const started = performance.now();
    const handle = this.#motor.build(ingestJson);
    const buildMs = performance.now() - started;
    this.#handle = handle;
    return { handle, buildMs };
  }

  /** Runs `layoutId` over the live handle and copies the whole run out. */
  run(layoutId: string, buildMs: number): RunReport {
    const handle = this.#handle;
    if (handle === null) throw new GraphMotorError("no graph has been built yet");
    const started = performance.now();
    const result = this.#motor.layout(handle, layoutId);
    const layoutMs = performance.now() - started;
    const columns = this.#readColumns(handle, result.nodeKind, result.edgeKind);
    return layoutReport(layoutId, result, { buildMs, layoutMs }, columns);
  }

  /**
   * Runs the POST capability `postId` over the SAME handle, immediately after
   * `base`'s layout, and copies the whole run out again: a pass REPLACES the
   * handle's edge geometry and may change its kind, so the columns — and the two
   * geometry tags — are read after it, not before.
   *
   * The pass reads the layout's own edges, never `base`'s post-pass edges
   * (`docs/contract/wasm-abi.md` "POST"), so two passes in a row are the same as
   * the second alone; the studio therefore reports only the last one.
   */
  post(base: RunReport, postId: string): RunReport {
    const handle = this.#handle;
    if (handle === null) throw new GraphMotorError("no graph has been built yet");
    const started = performance.now();
    const result: PostResult = this.#motor.post(handle, postId);
    const postMs = performance.now() - started;
    const columns = this.#readColumns(handle, result.nodeKind, result.edgeKind);
    return withPost(base, result, columns, postMs);
  }

  /**
   * Runs the analysis `analysisId` over the live handle's topology and returns
   * the ABI's parsed face. No layout run is required — every analysis is a pure
   * function of the topology — so this works straight after {@link build}.
   */
  analysis(analysisId: string): AnalysisResult {
    const handle = this.#handle;
    if (handle === null) throw new GraphMotorError("no graph has been built yet");
    return this.#motor.analysis(handle, analysisId);
  }

  release(): void {
    if (this.#handle === null) return;
    this.#motor.release(this.#handle);
    this.#handle = null;
  }

  /** Read every column this run can carry, in one synchronous block, copying as
   *  we go: the next `Motor` call would invalidate every view in here (C7). */
  #readColumns(
    handle: Handle,
    nodeKind: NodeGeometryKind,
    edgeKind: EdgeGeometryKind,
  ): ColumnInput {
    const read = (id: ColumnId): Column => this.#motor.column(handle, id);
    return {
      nodeKind, edgeKind,
      x: read(ColumnId.NodeX) as Float32Array | null,
      y: read(ColumnId.NodeY) as Float32Array | null,
      r: read(ColumnId.NodeR) as Float32Array | null,
      w: read(ColumnId.NodeW) as Float32Array | null,
      h: read(ColumnId.NodeH) as Float32Array | null,
      source: read(ColumnId.EdgeSource) as Uint32Array | null,
      target: read(ColumnId.EdgeTarget) as Uint32Array | null,
      offsets: read(ColumnId.EdgeOffsets) as Uint32Array | null,
      pts: read(ColumnId.EdgePts) as Float32Array | null,
      curveDegree: read(ColumnId.EdgeCurveDegree) as Uint32Array | null,
    };
  }
}
