// Every registered analysis, plus the fixture-specific value checks.

import {
  AnalysisRefusedError,
  InvalidHandleError,
} from "../../crates/graph-sdk-js/src/index.ts";
import { check, refusedWith } from "./lib.mjs";

// The contract's own element-type table, restated per id the way `post.mjs:51-59` restates
// the edge-kind table. Checked as a two-value set it caught nothing: the `u32` branch only
// adds an integrality check the f64 values satisfy, so `analysis::registry::to_json` could
// declare `analysis.centrality.closeness` a `u32` and every loop check would pass (M34).
const ANALYSIS_KINDS = new Map([
  ["analysis.centrality.closeness", "f64"],
  ["analysis.centrality.degree", "f64"],
  ["analysis.centrality.eigenvector", "f64"],
  ["analysis.communities.louvain", "u32"],
  ["analysis.components.weak", "u32"],
  ["analysis.components.strong", "u32"],
  ["analysis.depth.bfs", "u32"],
  ["analysis.centrality.betweenness", "f64"],
]);

/** Why a face's declared element type is not the one the table names for `id`, or `null`.
 *  Exported so the table itself can be pinned: the review's failing input is a producer
 *  that declares `analysis.centrality.closeness` a `u32`, which the old two-value check
 *  accepted because its f64 values satisfy the `u32` branch's integrality test. */
export function elementTypeProblem(id, kind) {
  const want = ANALYSIS_KINDS.get(id);
  if (want === undefined) return `${id} is not named in the element-type table`;
  return kind === want ? null : `declared ${kind}, contract says ${want}`;
}

export async function runAnalysisSection(ctx) {
  const { motor, stagedHandle, posts } = ctx;
// The analyses, over a graph with **no layout run at all** — the maths is a function of
// the topology, so this is a supported state and not a refusal.
const analyses = typeof motor.analyses === "function" ? motor.analyses() : [];
check("the SDK publishes the module's ANALYSIS registry (C1)", analyses.length > 0);
check("the ANALYSIS registry names the eight registered analyses", analyses.length === 8 && new Set(analyses).size === 8);
let analysed = 0;
for (const analysisId of analyses) {
  // A refusal here is a real failure (a face that does not name itself, a value of the
  // wrong type, a length that disagrees with `nodeCount`), so it is *caught* and reported
  // as `not ok` rather than left to abort the script: a crash would take every check
  // after this loop with it, hiding whatever else is broken.
  let result;
  try {
    result = motor.analysis(stagedHandle, analysisId);
  } catch (error) {
    check(`${analysisId}: the published SDK returns a typed result`, false);
    process.stdout.write(`#   ${String(error)}\n`);
    continue;
  }
  analysed += 1;
  process.stdout.write(`# ${analysisId}: ${result.kind} x ${result.nodeCount} = ${JSON.stringify(result.values)}\n`);
  check(`${analysisId}: it names itself`, result.id === analysisId);
  check(`${analysisId}: one value per node, in order`, result.values.length === 5);
  // The face's own node count is asserted, not just its value count: a face carrying
  // `"nodeCount": 3` with five values parses and would otherwise pass (m92).
  check(`${analysisId}: it reports the fixture's five nodes`, result.nodeCount === 5, String(result.nodeCount));
  check(
    `${analysisId}: its element type is the one the contract names for this id`,
    elementTypeProblem(analysisId, result.kind) === null,
    elementTypeProblem(analysisId, result.kind) ?? "",
  );
  check(`${analysisId}: its values are finite`, result.values.every(Number.isFinite));
  if (result.kind === "u32") {
    check(`${analysisId}: a labelling is whole and non-negative`, result.values.every((v) => Number.isInteger(v) && v >= 0));
  }
  // The three escape hatches, each present exactly when the analysis hands one back.
  if (analysisId === "analysis.centrality.eigenvector") {
    check("the eigenvector reports its convergence flag, not just its numbers", typeof result.converged === "boolean");
  } else {
    check(`${analysisId}: carries no convergence flag`, result.converged === undefined);
  }
  if (analysisId === "analysis.communities.louvain") {
    check("Louvain reports the modularity of the partition it returned", Number.isFinite(result.modularity));
  } else {
    check(`${analysisId}: carries no modularity`, result.modularity === undefined);
  }
  if (analysisId === "analysis.depth.bfs") {
    check("depth reports the deepest level it reached", Number.isInteger(result.max) && result.max >= 0);
  } else {
    check(`${analysisId}: carries no depth maximum`, result.max === undefined);
  }
}
check("every registered analysis ran through the published SDK", analysed === analyses.length && analysed > 0);
// Every registered analysis is named in the element-type table above, so a registry that
// grew past it is caught here rather than silently unchecked in the loop.
check(
  "every registered analysis is named in the element-type table",
  analyses.every((id) => ANALYSIS_KINDS.has(id)),
  analyses.filter((id) => !ANALYSIS_KINDS.has(id)).join(", "),
);
check(
  "the element-type table names no analysis the registry does not",
  [...ANALYSIS_KINDS.keys()].every((id) => analyses.includes(id)),
  [...ANALYSIS_KINDS.keys()].filter((id) => !analyses.includes(id)).join(", "),
);
// The checks below read two named results, so they only run if both were produced.
if (analysed === 0) {
  // The loop's own checks said nothing could be read; the handle must still go back, or a
  // section that never got to run leaks it (m91).
  motor.release(stagedHandle);
  check("an analysis ran, so its values could be read at all", false);
} else {

// The component labellings, read as the contract states them: a graph with two roots
// (`{a,b,c}` and `{d,z}`) is two components, numbered by the first member seen ascending.
check(
  "the weak components of the fixture are the two roots, numbered by first member",
  (() => {
    const r = motor.analysis(stagedHandle, "analysis.components.weak");
    return r.kind === "u32" && r.values[0] === 0 && r.values[4] === 1;
  })(),
);
check(
  "the hierarchy depth of the fixture counts out from its roots",
  (() => {
    const r = motor.analysis(stagedHandle, "analysis.depth.bfs");
    // `b` is the only node with a parent (it has the `h0` hierarchy edge to `d`), so
    // `a`, `b`, `c` and `z` are all roots — four roots, which hang off the hidden
    // virtual root, putting every one of them at depth 1 and `d` (a child of `b`) at 2.
    return (
      r.kind === "u32" &&
      r.values[0] === 1 && r.values[1] === 1 && r.values[2] === 1 &&
      r.values[3] === 2 && r.values[4] === 1 &&
      r.max === 2
    );
  })(),
);
// The stage results the SDK hands back are its own, not a re-derivation: the same
// handle, the same ids, asked twice, must answer identically — and a pass run over a
// *freshly laid out* handle must match one run over the handle the loop left behind, or
// "idempotent" would be a claim nothing checks.
check("the same post pass twice answers the same edges", (() => {
  const first = motor.layout(stagedHandle, "layout.grid") && motor.post(stagedHandle, "post.style.bezier");
  const a = JSON.stringify(motor.toJSON(stagedHandle));
  motor.layout(stagedHandle, "layout.grid");
  motor.post(stagedHandle, "post.style.bezier");
  const b = JSON.stringify(motor.toJSON(stagedHandle));
  return first !== null && a === b;
})());
check(
  "the degree centrality agrees with the topology it was computed from",
  (() => {
    const r = motor.analysis(stagedHandle, "analysis.centrality.degree");
    // `b` is incident to `r0`, the parallel `r2`, `r1` and the hierarchy edge `h0`; `a`
    // to `r0`, `r2` and its own self-loop, which a degree column counts **twice** (it is
    // both endpoints); `c` only to `r1`; `d` only to `h0`; `z` to nothing.
    return (
      r.kind === "f64" && r.values[0] === 4 && r.values[1] === 4 &&
      r.values[2] === 1 && r.values[3] === 1 && r.values[4] === 0
    );
  })(),
);
check("an unknown analysis id is refused", await refusedWith(AnalysisRefusedError, () => motor.analysis(stagedHandle, "analysis.components.none")));
// A bare `catch { refused = true }` accepts *any* throw: a `RangeError`, a
// `MotorTrapError`, or the wrong refusal class all reported ok. The contract names the
// code for a released handle — `InvalidHandle` (`docs/contract/wasm-abi.md:47`) — and that
// is what is asserted here, not the stage's own refusal class (M35).
const releasedHandle = async () => {
  motor.release(stagedHandle);
  const codes = await Promise.all(
    [
      ["post", () => motor.post(stagedHandle, posts[0])],
      ["analysis", () => motor.analysis(stagedHandle, analyses[0])],
    ].map(async ([stage, run]) => {
      try {
        await run();
        return `${stage} did not throw`;
      } catch (error) {
        return error instanceof InvalidHandleError && error.codeName === "InvalidHandle"
          ? null
          : `${stage} refused with ${error?.constructor?.name}(codeName ${error?.codeName})`;
      }
    }),
  );
  return codes.every((problem) => problem === null) ? true : codes.filter(Boolean).join("; ");
};
check("a released handle is refused by both stages as InvalidHandle (C6)", await releasedHandle());
}
}
