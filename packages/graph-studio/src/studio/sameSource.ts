/**
 * Whether a settings change names a new graph or the same one, which decides between a load and a
 * restyle. Its own module so the pipeline stays about running the stages.
 */
import type { Source } from "../state/settings.ts";

/**
 * Whether two sources name the same graph, member by member.
 *
 * Not `JSON.stringify`: a `document` source carries the whole document text, so serialising both
 * sides to compare them built two strings as long as the document on every settings change. Every
 * member is a string or a number, so `===` answers it and allocates nothing.
 */
export function sameSource(a: Source, b: Source): boolean {
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
  return a.kind === "document" && b.kind === "document" && a.name === b.name && a.text === b.text;
}
