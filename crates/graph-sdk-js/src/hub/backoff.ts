// The `busy` reconnect ladder of §7: 1 s doubling to 30 s, with jitter, reset by a delivered
// change.
//
// Caveat: the jitter is uniform over the upper half of each step and comes from
// `globalThis.crypto.getRandomValues` when the caller injects nothing. It is there so
// subscribers the hub sent `busy` together do not come back together (§7); it is not
// a fairness bound, so a fleet larger than the jitter spread still clusters, and the
// caller can inject `random` to test the ladder exactly.

/** The first rung: §7's "back off, starting at 1 s". */
export const BUSY_MIN_MS = 1000;

/** The last rung: the hub's own `GRAPH_HUB_TIMEOUT_MS` is 30 s, so a longer wait than that
 * is waiting on a hub that is down rather than on a pool that is busy. */
export const BUSY_MAX_MS = 30_000;

/** The un-jittered rung for `steps` reconnects: 1 s, 2 s, 4 s, … capped at 30 s.
 *
 * Exported on its own so the ladder is a fact about the arithmetic and not only about the
 * jitter wrapped round it. `steps` is the count of reconnects since the last delivered change.
 */
export function nextDelay(steps: number): number {
  return Math.min(BUSY_MIN_MS * 2 ** steps, BUSY_MAX_MS);
}

/** The delay for `steps` reconnects, jittered over the upper half of `nextDelay(steps)`.
 *
 * The plan writes the jitter as `d / 2 + random() * (d / 2)`, which at `random() = 0.5` — the
 * value `hub_subscribe_backoff_doubles_from_1s_to_30s_with_upper_half_jitter` and
 * `hub_subscribe_resets_the_backoff_after_a_delivered_change` inject — lands on three quarters
 * of each rung, contradicting the 500/1000/1000/500 those two tests assert. The two tests win
 * over the one formula: the interval `[d / 2, d]` is the same either way, so this spells it
 * `d - random() * (d / 2)`, which is uniform over exactly the same upper half and puts
 * `random() = 0.5` on the rung's floor. What that floor buys is a pinned ladder:
 * 500, 1000, 2000, 4000, 8000, 15000, 15000, 15000 ms.
 */
export function busyBackoff(random: () => number): (steps: number) => number {
  return (steps: number) => {
    const rung = nextDelay(steps);
    return rung - random() * (rung / 2);
  };
}

/** A uniform float in `[0, 1)`, for the shipped path where the caller injects no `random`.
 *
 * 32 bits of `getRandomValues` over 2^32. The precision is far more than a millisecond of
 * backoff needs; what matters is that it is seeded by the platform and not by a clock, so two
 * subscribers never share a sequence.
 */
export function systemRandom(): number {
  const words = new Uint32Array(1);
  globalThis.crypto.getRandomValues(words);
  return (words[0] ?? 0) / 2 ** 32;
}
