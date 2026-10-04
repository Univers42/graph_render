// The `--split` half of the wasm32 columnar arm: `encodeBatch` under a timer of its own, beside
// the unchanged `extend` timer, so `extend - encode` is everything the JS encoder does *not*
// explain — the copy into linear memory plus the decode and index walk inside wasm32.
// `docs/measurements/perf-p4f-wasm.md` is the measurement this exists for; A1 in
// `docs/decisions/extend-columns.md` is the question it answers.
//
// The extra encode is run once per batch and its bytes are thrown away: the timed
// `extendColumns` right below encodes again, unchanged, because an arm that stopped charging
// the host its own encoder would be measuring something no host pays (the same asymmetry the
// native arm has, and for the same reason).

import { readFileSync } from "node:fs";
//
// Caveat: the split arm's wall time per batch is the sum of both encodes, so it is roughly
// double the default arm's, and each batch leaves a second short-lived byte array for the
// collector. The `extend` column is unaffected — it is measured around its own call — but the
// host is doing twice the GC work, so the split arm is not a control for the default one.

/** The batch table under `--split`: `encode` beside `extend` and `grow`. */
export const SPLIT_BATCH_HEADER =
  "| batch | nodes after | encode ms | extend ms | grow ms | sum ms |\n" +
  "|---:|---:|---:|---:|---:|---:|";

/** The summary row under `--split`: `encode` in median, p95 and max, the rest as the default. */
export const SPLIT_SUMMARY_HEADER =
  "| layout | batch | n | encode median ms | encode p95 ms | encode max ms | extend median ms " +
  "| grow median ms | sum median ms | sum p95 ms | sum max ms | load start | load end |\n" +
  "|---|---|---|---|---|---|---|---|---|---|---|---|---|";

/**
 * `encodeBatch` alone, in one timer, with the bytes handed back only so the caller can see how
 * many there were — the encoder's result is not the measurement, and a discarded array is still
 * one the collector has to reclaim.
 */
export function timeEncode(encodeBatch, at) {
  const started = performance.now();
  const bytes = encodeBatch(at);
  const elapsed = performance.now() - started;
  return { ms: elapsed, bytes: bytes.length };
}

/** One batch row under `--split`: the same cells as the default, with `encode` in front. */
function batchRow(row, index) {
  const sum = row.extendMs + row.growMs;
  return `| ${index + 1} | ${row.nodesAfter} | ${row.encodeMs.toFixed(2)} | ` +
    `${row.extendMs.toFixed(2)} | ${row.growMs.toFixed(2)} | ${sum.toFixed(2)} |`;
}

/**
 * The four markdown pieces the bench prints, in the same order as the default arm. `stats` is
 * the bench's own `median`/`quantile`/`max`, passed in rather than copied: the two arms'
 * columns are comparable only if the three statistics are computed one way, and the file that
 * computes them for the default arm is the one that must compute them for this one.
 */
export function splitTables(run, stats, label) {
  const encode = run.rows.map((r) => r.encodeMs);
  const sums = run.rows.map((r) => r.extendMs + r.growMs);
  const column = (values) => stats.median(values).toFixed(2);
  const cells = [
    column(encode), stats.quantile(encode, 0.95).toFixed(2), stats.max(encode).toFixed(2),
    column(run.rows.map((r) => r.extendMs)), column(run.rows.map((r) => r.growMs)),
    column(sums), stats.quantile(sums, 0.95).toFixed(2), stats.max(sums).toFixed(2),
  ];
  return {
    batchHeader: SPLIT_BATCH_HEADER,
    batchRows: run.rows.map(batchRow).join("\n"),
    summaryHeader: SPLIT_SUMMARY_HEADER,
    summaryRow: `| ${label} | ${run.batchNodes} | ${run.nodes} | ${cells.join(" | ")} | ` +
      `${run.loadStart} | ${loadEnd()} |`,
  };
}

/** `/proc/loadavg`'s first three fields at the end of the replay, or `unavailable`. */
function loadEnd() {
  try {
    return readFileSync("/proc/loadavg", "utf8").split(/\s+/).slice(0, 3).join(" ");
  } catch {
    return "unavailable";
  }
}
