/**
 * The largest graph the studio tries to build, checked before anything is allocated for it.
 * Past these a worker spends gigabytes and then throws, or the page's heap runs out; the
 * numbers are in docs/measurements/memory-profile.md, section "Studio".
 */

/**
 * A document's length. A 97 MiB document peaked at 791 MiB to normalise and a 493 MiB one at
 * 3.5 GiB, near a browser worker's heap; the normalised rewrite of a document is about as long
 * as the document, and V8 builds no string past 2^29 characters, so the cap is half that.
 *
 * Caveat: counts UTF-16 code units, what a JS string holds. A file is checked by its byte size
 * before it is read; UTF-8 never has fewer bytes than code units, so a mostly non-ASCII file
 * just under the cap in characters can still be refused by its bytes.
 */
export const MAX_DOCUMENT_CHARS = 2 ** 28;

/**
 * Links a generated graph may ask for, nodes × links per node. A million nodes at two links is
 * a 517 M-character document, 96 % of V8's longest string; at three links the worker throws
 * `Invalid string length` after allocating about 2 GiB.
 */
export const MAX_LINKS = 2_000_000;

const MIB = 2 ** 20;

/** Why a document of `chars` characters (or bytes, for a file not yet read) is refused, or null. */
export function documentRefusal(chars: number): string | null {
  if (chars <= MAX_DOCUMENT_CHARS) return null;
  return `is ${(chars / MIB).toFixed(1)} Mi characters; the studio opens at most ${MAX_DOCUMENT_CHARS / MIB} Mi`;
}

/** Why a generated graph of `nodes` × `degree` is refused, or null. */
export function linksRefusal(nodes: number, degree: number): string | null {
  const links = nodes * degree;
  if (links <= MAX_LINKS) return null;
  return `${nodes} nodes × ${degree} links per node is ${links} links; the studio generates at most ${MAX_LINKS}`;
}
