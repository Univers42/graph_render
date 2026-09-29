/** The search's ranking: labels that start with the text, then labels that hold it. */

/**
 * Ponytail: the text is not trimmed, so a pasted line with a space at either end matches
 * nothing and the list goes empty rather than showing something near. Escape hatch: clear
 * the field and type the word again.
 */
export function matchesOf(labels: readonly string[], text: string, limit = 8): readonly number[] {
  if (text === "") return [];
  const wanted = text.toLowerCase();
  // One pass, two buckets: sorting every label of a large graph to keep eight would be the
  // most expensive thing in the chrome.
  const starts: number[] = [];
  const holds: number[] = [];
  for (let i = 0; i < labels.length; i += 1) {
    const at = (labels[i] ?? "").toLowerCase().indexOf(wanted);
    if (at === 0) starts.push(i);
    else if (at > 0) holds.push(i);
  }
  return [...starts, ...holds].slice(0, limit);
}
