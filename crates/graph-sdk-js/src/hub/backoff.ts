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

/** The delay for `steps` reconnects, jittered over the upper half of `nextDelay(steps)`:
 * uniform on `[rung / 2, rung]`, so the floor is half the rung and the ceiling is the rung.
 *
 * The plan says the two tests that pin the ladder inject `random() = 0.5`. They cannot:
 * `0.5` lands on three quarters of each rung — 750, 1500, 3000, 6000, 12000, 22500, 22500,
 * 22500 — which contradicts the very list that paragraph pins (500, 1000, 2000, 4000, 8000,
 * 15000, 15000, 15000) and the 500/1000/500/1000 `hub_subscribe_resets_the_backoff_after_a_
 * delivered_change` asserts. The formula below is the plan's, verbatim; the tests inject
 * `random() = 0`, which is the value that lands on the list.
 *
 * The list's 16000 in sixth place is unreachable at any injection: a 30 s cap puts the ceiling
 * at 15000, and 500 · 2^5 = 16000 is already past it. The cap is 30 s because the hub's own
 * `GRAPH_HUB_TIMEOUT_MS` is 30 s, so the test name's "to 30s" and the ladder agree at 15000.
 */
export function busyBackoff(random: () => number): (steps: number) => number {
  return (steps: number) => {
    const rung = nextDelay(steps);
    return rung / 2 + random() * (rung / 2);
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
