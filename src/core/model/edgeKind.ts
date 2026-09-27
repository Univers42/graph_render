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
 *
 * PONYTAIL: this is a heuristic, not a parser, and it is exact in the wrong places.
 *
 *  - The hierarchy test is three exact matches plus a `"hierarchy"` substring. It
 *    is NOT a `"parent"` substring, so `"parent_tag"` classifies as `tag`, not
 *    `hierarchy`. Pinned by a test so a future "widening" is deliberate.
 *  - Three of the five branches match on a *substring anywhere* in the type. A
 *    field named `"contested"` contains `"test"`… and `"tags_in_review"` matches
 *    `tag` before any later branch could claim it. Branch order is therefore
 *    load-bearing and not obviously reorderable.
 *  - **Unknown types degrade silently to `relation`.** There is no error, no
 *    warning, and no log. A renamed or newly invented wire type renders as a
 *    plain structural edge that looks intentional, so the misclassification is
 *    undetectable from the output. That is the same failure shape as the
 *    unresolved-theme-token problem in README § Limitations: a wrong-but-plausible
 *    render with nothing to detect it.
 *  - Extracted verbatim from the host, whose behaviour this preserves. Any
 *    tightening is a behaviour change for existing hosts and is not a bug fix.
 */
export function edgeKindFromType(type: string | undefined): EdgeKind {
  if (!type) return "relation";
  const lowered = type.toLowerCase();

  // Hierarchy: exact literals plus a "hierarchy" substring. "hierarchy" is an
  // engine-generated marker, so the substring is safe here — no ordinary English
  // word a user would name a field after contains it.
  if (lowered === "parent" || lowered === "parent_of" || lowered === "child_of" || lowered.includes("hierarchy")) {
    return "hierarchy";
  }

  // Note relations: engine-generated markers, so substring matching is safe.
  if (lowered.includes("note_link") || lowered === "links_to") return "note_link";
  if (lowered.includes("note_of") || lowered === "annotates") return "note_of";

  // Tag: EXACT literals only. This is the one branch that must not substring-match.
  //
  // A previous version used `lowered.includes("tag")` here, on the reasoning that
  // tag edges are named after their tag. That is wrong because the wire `type` is
  // not a closed enum — the graph contract states it is "your explicit types, plus
  // note_link, tagged, and <field-name> for references", and a relation property is
  // free text chosen by whoever built the database. `includes("tag")` therefore
  // claimed, among others:
  //
  //     vintage    heritage    advantage    montage    frontage    cottage    stage
  //
  // (he-ri-TAG-e, advanTAG-e, monTAG-e, fronTAG-e, cotTAG-e, sTAG-e) — all ordinary
  // field names for a product catalogue, an estate, or a photography archive. Those
  // edges would be painted with the tag colour, bucketed into the tag geometry
  // tier, and matched by the tag filter predicate. A user would see miscoloured
  // edges with no error anywhere.
  //
  // Only `tagged` and `tag` are contract literals, so only those are matched. A
  // user-defined reference field lands on `relation`, which is the documented
  // default and the visually neutral choice.
  if (lowered === "tagged" || lowered === "tag") return "tag";

  return "relation";
}
