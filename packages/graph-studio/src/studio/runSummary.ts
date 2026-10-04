/**
 * What the studio reports about a run: its ids, its sizes, and the notes the motor left on the
 * snapshot, counted by name so a thousand identical notes read as one line.
 */
import type { Snapshot } from "../../../graph-render/src/snapshot/decode.ts";
import type { RunReport } from "../motor/protocol.ts";
import type { RunSummary } from "../state/model.ts";

function degradations(snapshot: Snapshot): string[] {
  const counts = new Map<string, number>();
  for (const note of snapshot.notes) {
    const name = note.name ?? `note ${note.code}`;
    counts.set(name, (counts.get(name) ?? 0) + 1);
  }
  return [...counts].map(([name, count]) => (count === 1 ? name : `${name} ×${count}`));
}

export function summaryOf(run: RunReport, snapshot: Snapshot): RunSummary {
  const refused = run.postError === null ? [] : [`${run.postError.title}: ${run.postError.detail} — the layout's own edges are shown`];
  return {
    layoutId: run.layoutId, postId: run.postId, postError: run.postError, digest: run.digest,
    byteLength: run.bytes.byteLength, nodeKind: snapshot.nodeKind, edgeKind: snapshot.edgeKind,
    // The dim off the decoded snapshot, not off the layout id: the z column's presence is
    // what the painter branches on, so that is what the badge has to report.
    dim: snapshot.dim,
    layoutMs: run.layoutMs, postMs: run.postMs, notes: [...degradations(snapshot), ...refused],
  };
}
