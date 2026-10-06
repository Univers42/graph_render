/**
 * What a restyle is made of, held to recognise the call that has nothing to do. The drawn style is
 * a pure function of these six values (`styleInputOf`, `styleFrom`), the state replaces rather
 * than mutates them, and `restyle` is the studio's one caller of `view.setStyle`, so while all six
 * are the same objects the view's style is current. A layout switch restyled with none of them
 * changed and paid an O(n) style and scene rebuild on the first frame of its move.
 */
import type { AnalysisReport } from "../../motor/protocol.ts";
import type { GraphMeta } from "../../source/meta.ts";
import type { Appearance, Filter, Group } from "../../state/settings.ts";

export interface StyleIn {
  readonly meta: GraphMeta;
  readonly appearance: Appearance;
  readonly filter: Filter;
  readonly groups: readonly Group[];
  readonly analysis: AnalysisReport | null;
  readonly reveal: number | null;
}

export function sameStyleIn(held: StyleIn | null, next: StyleIn): boolean {
  return held !== null && held.meta === next.meta && held.appearance === next.appearance
    && held.filter === next.filter && held.groups === next.groups
    && held.analysis === next.analysis && held.reveal === next.reveal;
}
