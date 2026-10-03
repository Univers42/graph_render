/**
 * One entry per part: the inputs it was last given, and what they produced. A reveal step
 * changes none of a part's inputs, so the part hands back the array it already built and the
 * step costs what the reveal moved.
 *
 * Keys are compared by identity, member by member, with `===`. The studio holds one meta, one
 * analysis, one filter and one group list at a time and replaces a member by giving it a new
 * object (`state/settings.ts` freezes every document it writes), so a different value is
 * always a different object and never a mutated one. A key that compares equal therefore means
 * the same values, not merely equal ones.
 *
 * Nothing is written to a held result. The studio's own `withReveal` copies the mask before it
 * hides a row in it (`look/reveal.ts`), and every reader of `weights`, `colours` and `hidden`
 * downstream only reads them — the two writers of a hidden mask, `graph-render/src/canvas2d/keep.ts:20`
 * and `graph-render/src/local.ts:39`, both fill a mask of their own first. So the arrays
 * a memo hands out may be held by the renderer across steps, which is what `webgl2/sync.ts:48`
 * already assumes when it skips a re-upload of an unchanged `colours`.
 *
 * Ponytail: one entry, not a map. A second entry is a second copy of what the studio's state
 * already holds, and at a million nodes that is a megabyte-sized array kept alive for a
 * comparison that will never come. Direction it errs: two studios on one page, or one studio
 * alternating between two looks, each recompute and lose the saving — that is the cost this
 * removes coming back, never a wrong number. Escape hatch: none needed; a caller that wants a
 * part rebuilt says so by passing a new object, which is how every other cache in `src/` is
 * invalidated too.
 */
export interface Memo<Value> {
  /**
   * The value for `key`, and `build` called only when the held entry's key is a different one.
   * `build` reads the caller's own variables, so the key has to name every one of them.
   */
  read(key: readonly unknown[], build: () => Value): Value;
}

/** Whether two keys are the same inputs: the same members, in the same order, by identity. */
function sameKey(held: readonly unknown[], asked: readonly unknown[]): boolean {
  if (held.length !== asked.length) return false;
  for (let i = 0; i < held.length; i += 1) if (held[i] !== asked[i]) return false;
  return true;
}

interface Entry<Value> {
  readonly key: readonly unknown[];
  readonly value: Value;
}

export function memo<Value>(): Memo<Value> {
  let held: Entry<Value> | null = null;
  return {
    read(key, build) {
      const found = held;
      if (found !== null && sameKey(found.key, key)) return found.value;
      const built: Entry<Value> = { key, value: build() };
      held = built;
      return built.value;
    },
  };
}
