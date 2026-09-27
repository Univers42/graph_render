/**
 * Classify a wire edge `type` string into the engine's internal `EdgeKind`.
 *
 * The package exports the `EdgeKind` union (`core/types.ts`) and three modules
 * switch on it to pick colours and bucket geometry — but until this existed,
 * nothing could *produce* a kind, so every caller had to either carry a
 * hand-rolled classifier or switch on raw strings at the point of use.
 *
 * Extracted verbatim from osionos `src/features/second-brain/model/edges/edgeKind.ts`.
 * Host-neutral on purpose: it takes a plain `string | undefined`, so it does not
 * import the BaaS wire types, and it stays inside the `core/` firewall.
 */

import type { EdgeKind } from "../types";

/**
 * Map a wire edge `type` string to our internal `EdgeKind`. Covers the explicit
 * edges-mount types plus the server-side generator types documented in the graph
 * contract (`note_link`, `tagged`, and `<field-name>` references). Unknown types —
 * including reference edges named after a field — fall through to `relation`,
 * our default structural edge.
 */
export function edgeKindFromType(type: string | undefined): EdgeKind {
  if (!type) return "relation";
  const lowered = type.toLowerCase();
  if (lowered === "parent" || lowered === "parent_of" || lowered === "child_of" || lowered.includes("hierarchy")) return "hierarchy";
  if (lowered.includes("note_link") || lowered === "links_to") return "note_link";
  if (lowered.includes("note_of") || lowered === "annotates") return "note_of";
  if (lowered === "tagged" || lowered === "tag" || lowered.includes("tag")) return "tag";
  return "relation";
}
