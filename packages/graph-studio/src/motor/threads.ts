/**
 * How many threads tick a live settle, the motor worker's own included. More than one loads the
 * threads artifact over a pool of helper Workers (`worker.ts`); that needs a shared memory, which a
 * browser gives only to a cross-origin isolated page. One is the serial module.
 */

/**
 * Ponytail: at most 8, and one core left to the page. Eight workers gain only 1.16x over four on a
 * 400k tick (`docs/measurements/perf-p3-session-threads.md`), and each helper holds a 4 MiB stack.
 * Failing input: a host with more than nine free cores leaves the rest idle, and a host whose
 * cores are busy elsewhere gets more threads than it can run. Escape hatch: `?threads=N`.
 */
export const MAX_THREADS = 8;

export function threadsFor(asked: number | undefined, cores: number, isolated: boolean): number {
  if (!isolated) return 1;
  const wanted = Math.trunc(asked ?? cores - 1);
  return Number.isFinite(wanted) ? Math.min(Math.max(wanted, 1), MAX_THREADS) : 1;
}
