/**
 * The page's half of `applyDeltas`: the structure snapshot the worker rebuilt after a batch,
 * and the live frame drawn up to the node count that snapshot describes.
 *
 * `docs/contract/delta.md` §"The studio": pick and hover read the frame the studio hands them,
 * so drawing `xs.subarray(0, nodeCount)` makes them ignore the rows past it and the renderer
 * needs no append path. The count is the last snapshot's, so between two snapshots the rows
 * past it exist in the motor and are not on screen.
 */
import { frameFrom } from "../../../graph-render/src/frame.ts";
import { decodeSnapshot } from "../../../graph-render/src/snapshot/decode.ts";
import type { View } from "../../../graph-render/src/view.ts";
import type { GraphMeta } from "../source/meta.ts";
import type { RunReport } from "./protocol.ts";

/** The two members of the view this needs; the studio hands the page the same view. */
export type DeltasView = Pick<View, "setFrame" | "setPositions">;

export interface DeltasPage {
  /** The structure snapshot the worker rebuilt after a batch: the new nodes, drawn. */
  readonly structure: (run: RunReport) => void;
  /** One live frame, drawn up to the node count the last snapshot described. */
  readonly frame: (xs: Float32Array, ys: Float32Array) => void;
  /** How many nodes the drawing holds, or null before the first snapshot. */
  readonly drawn: () => number | null;
}

export function createDeltasPage(view: DeltasView, described: () => GraphMeta | null, remember: (next: GraphMeta) => void): DeltasPage {
  const drawn = (): number | null => described()?.nodeCount ?? null;

  /**
   * Caveat: a snapshot the worker could not describe — no ids, or ids of another size — is not
   * drawn and not remembered, so the page keeps the structure it had and the batch's nodes wait
   * for the next rebuild. Failing input: the batch's own ids, which is the only case the motor
   * can produce, are described; a mismatch is a studio bug and shows as nodes that never appear.
   * Direction: the worker rebuilds the structure at most twice a second, so this is called at
   * most twice a second and not once per batch. Escape hatch: a re-layout draws the whole graph.
   */
  const structure = (run: RunReport): void => {
    console.log("DELTA page structure", run.bytes.length, run.meta?.nodeCount ?? "no-meta");
    const snapshot = decodeSnapshot(run.bytes);
    const frame = frameFrom(snapshot);
    const meta = run.meta ?? described() ?? null;
    if (meta === null || meta.nodeCount !== frame.nodeCount) return;
    view.setFrame(frame, { animate: false });
    remember(meta);
  };

  /**
   * Caveat: between two structure snapshots the new nodes move in the motor and are not drawn;
   * the lag is at most one cadence period, 500 ms. Direction: the count is the snapshot's own,
   * so the drawn rows and the pick grid are the same rows. Escape hatch: a snapshot arrives
   * within a cadence period of any batch, so the lag is bounded and does not accumulate.
   */
  const frame = (xs: Float32Array, ys: Float32Array): void => {
    const count = drawn() ?? xs.length;
    view.setPositions(xs.subarray(0, count), ys.subarray(0, count));
  };

  return { structure, frame, drawn };
}
