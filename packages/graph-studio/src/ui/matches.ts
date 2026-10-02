/** The search's ranking: labels that start with the text, then labels that hold it. */

/**
 * Lowercased labels, keyed on the identity of the array they came from. Every layout replaces
 * `meta.labels` wholesale rather than editing it, so one array means one graph for its whole
 * life, and the keystrokes after the first read this instead of lowering thousands of labels
 * again. A `WeakMap` and not a `useMemo`: the index is a property of the labels, not of the
 * component, so every caller of the two ranking functions gets it and nothing has to be
 * remembered by a component that unmounts with the graph.
 *
 * Caveat: keyed on the identity of the array, so an equal-but-new array misses and is built
 * again, and an array edited in place would be ranked on the labels it used to hold.
 */
const INDEXES = new WeakMap<readonly string[], readonly string[]>();

/** The labels lowercased once, on the first keystroke that asks for them. */
export function loweredLabelsOf(labels: readonly string[]): readonly string[] {
  const held = INDEXES.get(labels);
  if (held !== undefined) return held;
  const lowered: string[] = [];
  // The `?? ""` is the same guard the ranking had: a hole in the array is an empty label.
  for (let i = 0; i < labels.length; i += 1) lowered.push((labels[i] ?? "").toLowerCase());
  INDEXES.set(labels, lowered);
  return lowered;
}

/**
 * Ponytail: the text is not trimmed, so a pasted line with a space at either end matches
 * nothing and the list goes empty rather than showing something near. Escape hatch: clear
 * the field and type the word again.
 */
export function allMatchesOf(labels: readonly string[], text: string): readonly number[] {
  if (text === "") return [];
  const wanted = text.toLowerCase();
  const lowered = loweredLabelsOf(labels);
  // One pass, two buckets: sorting every label of a large graph to keep eight would be the
  // most expensive thing in the chrome.
  const starts: number[] = [];
  const holds: number[] = [];
  for (let i = 0; i < lowered.length; i += 1) {
    const at = (lowered[i] ?? "").indexOf(wanted);
    if (at === 0) starts.push(i);
    else if (at > 0) holds.push(i);
  }
  return [...starts, ...holds];
}

/** The same ranking, cut at `limit` for the box: the mask is made of the whole of it. */
export function matchesOf(labels: readonly string[], text: string, limit = 8): readonly number[] {
  return allMatchesOf(labels, text).slice(0, limit);
}
