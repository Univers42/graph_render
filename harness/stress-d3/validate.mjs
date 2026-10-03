// The input contract of the d3-force arm, and the one output invariant graph-core relies
// on. Both are refusals: an arm that skips a case it did not measure, or writes a `null`
// where a coordinate belongs, hands the stress gate a comparison it cannot make sense of
// and cannot tell apart from a disagreement between two layouts.
//
// Everything here is pure, so `harness/stress-d3.test.mjs` can pin it without a node
// process, a d3 install or a wall clock.

/**
 * Why case `number` (0-based, as the line number in the input file) is not a case this
 * arm can run, or `null` when it is. The edge shape matters as much as the node count:
 * `[[0, 7]]` over two nodes reaches d3 as an unnamed `node not found: 7` from three
 * frames deep, with no case number and no hint which line was wrong.
 */
export function caseRefusal(number, input) {
  if (input === null || typeof input !== "object" || Array.isArray(input)) {
    return `case ${number}: the line is not a JSON object`;
  }
  const { seed, n, edges } = input;
  if (!Number.isInteger(seed)) return `case ${number}: no integer seed`;
  if (!Number.isInteger(n) || n < 0) return `case ${number}: no node count`;
  if (!Array.isArray(edges)) return `case ${number}: edges is not an array`;
  for (const [at, edge] of edges.entries()) {
    const where = `case ${number}: edge ${at}`;
    if (!Array.isArray(edge) || edge.length !== 2) return `${where} is not a [lo,hi] pair`;
    if (!edge.every((end) => Number.isInteger(end))) return `${where} is not a pair of integers`;
    if (!edge.every((end) => end >= 0 && end < n)) return `${where} names a node outside 0..${n - 1}`;
  }
  return null;
}

/**
 * Why the positions d3 reached for case `number` cannot be correlated, or `null`. JSON has
 * no NaN: `JSON.stringify({x: NaN})` is `{"x":null}`, so a non-finite coordinate reaches
 * `crates/graph-cli/src/stress/cases.rs` as a hole it rejects long after this arm is gone.
 * Catching it here names the case.
 */
export function positionsRefusal(number, x, y) {
  if (x.length !== y.length) return `case ${number}: ${x.length} x coordinates against ${y.length} y`;
  for (let i = 0; i < x.length; i += 1) {
    if (Number.isFinite(x[i]) && Number.isFinite(y[i])) continue;
    return `case ${number}: node ${i} reached a non-finite position (${x[i]}, ${y[i]})`;
  }
  return null;
}