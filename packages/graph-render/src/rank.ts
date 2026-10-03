/**
 * Node indices ordered heaviest first, ties in index order, in O(n): a stable LSD radix sort over
 * a 32-bit key per weight, where ascending key order is descending weight. It replaces a comparator
 * sort, `(w[b] - w[a]) || a - b`, that cost 230 ms at a million nodes against 17 ms for this one
 * (medians on a loaded host, `docs/measurements/perf-style-rank.md`).
 *
 * The sort is exact: it orders the f32 values themselves, so it needs no Caveat. Its one choice is
 * the input the comparator never ordered: NaN ranks as +0, which is also how `radiusFor` draws it,
 * and -0 ranks as +0, where the comparator already tied them.
 */

const DIGIT_BITS = 11;
const DIGITS = 1 << DIGIT_BITS;
const DIGIT_MASK = DIGITS - 1;
/**
 * Three 11-bit digits cover the 32-bit key. A pass whose digit is the same for every key is
 * skipped; on weights spread over 0..1 none is, since the top digit holds the exponent.
 */
const PASSES = 3;
const SIGN = 0x80000000;
const MAGNITUDE = 0x7fffffff;
const INFINITY_BITS = 0x7f800000;

/** The keys, one per node: `-0` and NaN take `+0`'s, then the f32 bits flip into a descending order. */
function keysOf(weights: Float32Array): Uint32Array {
  const bits = new Uint32Array(weights.buffer, weights.byteOffset, weights.length);
  const keys = new Uint32Array(weights.length);
  for (let i = 0; i < keys.length; i += 1) {
    const raw = bits[i] ?? 0;
    const u = raw === SIGN || (raw & MAGNITUDE) > INFINITY_BITS ? 0 : raw;
    const ascending = u >= SIGN ? ~u : u ^ SIGN;
    keys[i] = ~ascending;
  }
  return keys;
}

/** How many keys hold each value of each digit: pass `p` is `counts[p * DIGITS ..][..DIGITS]`. */
function countsOf(keys: Uint32Array): Uint32Array {
  const counts = new Uint32Array(PASSES * DIGITS);
  for (let i = 0; i < keys.length; i += 1) {
    const key = keys[i] ?? 0;
    for (let pass = 0; pass < PASSES; pass += 1) {
      const slot = pass * DIGITS + ((key >>> (pass * DIGIT_BITS)) & DIGIT_MASK);
      counts[slot] = (counts[slot] ?? 0) + 1;
    }
  }
  return counts;
}

/** Whether every key has the same digit in `pass`, which makes that pass a no-op. */
function isUniform(counts: Uint32Array, pass: number, total: number): boolean {
  for (let digit = 0; digit < DIGITS; digit += 1) {
    const count = counts[pass * DIGITS + digit] ?? 0;
    if (count !== 0) return count === total;
  }
  return true;
}

/** Where each digit's run starts in the output of `pass`. */
function startsOf(counts: Uint32Array, pass: number): Uint32Array {
  const starts = new Uint32Array(DIGITS);
  let sum = 0;
  for (let digit = 0; digit < DIGITS; digit += 1) {
    starts[digit] = sum;
    sum += counts[pass * DIGITS + digit] ?? 0;
  }
  return starts;
}

interface Pass {
  readonly keys: Uint32Array;
  readonly counts: Uint32Array;
  readonly pass: number;
}

/** One stable counting pass: `from`'s items into `to`, by digit, in the order `from` held them. */
function scatter({ keys, counts, pass }: Pass, from: Uint32Array, to: Uint32Array): void {
  const next = startsOf(counts, pass);
  const shift = pass * DIGIT_BITS;
  for (let at = 0; at < from.length; at += 1) {
    const item = from[at] ?? 0;
    const digit = ((keys[item] ?? 0) >>> shift) & DIGIT_MASK;
    const place = next[digit] ?? 0;
    to[place] = item;
    next[digit] = place + 1;
  }
}

export function rankByWeight(weights: Float32Array): Uint32Array {
  const keys = keysOf(weights);
  const counts = countsOf(keys);
  // Ties keep index order because every pass is stable and the first one reads the identity.
  let order = new Uint32Array(keys.length);
  for (let i = 0; i < order.length; i += 1) order[i] = i;
  let spare = new Uint32Array(keys.length);
  for (let pass = 0; pass < PASSES; pass += 1) {
    if (isUniform(counts, pass, keys.length)) continue;
    scatter({ keys, counts, pass }, order, spare);
    [order, spare] = [spare, order];
  }
  return order;
}
