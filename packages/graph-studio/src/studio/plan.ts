/**
 * What a settings change asks the motor for, and whether the two sources name the same graph.
 *
 * One decision, read once per `apply`, before anything is asked: a new source is loaded and laid
 * out, a new layout only laid out, a new look only restyled (`studio/pipeline.ts`). Both halves
 * are quiet — nothing is refused, the drawing is simply not what was asked for — so a member
 * `sameSource` forgets to compare reloads the graph, and one it compares wrongly keeps the old
 * graph on screen.
 */
import type { StudioState } from "../state/model.ts";
import type { Settings, Source } from "../state/settings.ts";

/**
 * Whether two sources name the same graph, member by member.
 *
 * Not `JSON.stringify`: a `document` source carries the whole document text, so serialising both
 * sides to compare them built two strings as long as the document on every settings change. Every
 * member is a string or a number, so `===` answers it and allocates nothing.
 */
function sameSource(a: Source, b: Source): boolean {
  if (a === b) return true;
  if (a.kind === "synthetic") return sameSynthetic(a, b);
  if (a.kind === "fixture") return sameFixture(a, b);
  return sameDocument(a, b);
}

// The kinds are disjoint, so each helper re-reads `b`'s kind rather than narrowing it against
// `a`'s: a union narrows on a discriminant it can see, and it cannot see this one.
function sameSynthetic(a: Source, b: Source): boolean {
  return a.kind === "synthetic" && b.kind === "synthetic"
    && a.seed === b.seed && a.nodes === b.nodes && a.degree === b.degree && a.shape === b.shape;
}

function sameFixture(a: Source, b: Source): boolean {
  return a.kind === "fixture" && b.kind === "fixture" && a.path === b.path;
}

function sameDocument(a: Source, b: Source): boolean {
  return a.kind === "document" && b.kind === "document"
    && a.name === b.name && a.text === b.text && a.host === b.host;
}

export interface Plan {
  readonly load: boolean;
  readonly layout: boolean;
  readonly analysis: boolean;
}

export function planOf(state: StudioState, next: Settings): Plan {
  const load = state.graph === null || !sameSource(state.settings.source, next.source);
  const { run } = state;
  // A filter that asks to be laid out again is one the last run was not made under, or the
  // layout would repeat the drawing already on screen for a filter nobody changed.
  const relayout = next.filter.relayout && JSON.stringify(next.filter) !== state.runFilter;
  // A value the last run was not made at is the same thing: the picture on screen is not the
  // one these settings ask for.
  const params = JSON.stringify(next.params[next.layout] ?? {});
  const layout = load || run === null || run.layoutId !== next.layout || run.postId !== next.edges
    || relayout || params !== state.runParams;
  const asked = next.analysis !== null && (load || state.analysis?.id !== next.analysis);
  return { load, layout, analysis: asked || (next.analysis === null && state.analysis !== null) };
}
