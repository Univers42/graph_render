// Every registered analysis, plus the fixture-specific value checks.

import {
  AnalysisRefusedError,
} from "../../crates/graph-sdk-js/src/index.ts";
import { check, refusedWith } from "./lib.mjs";

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
  check(`${analysisId}: its element type is one of the two the ABI names`, result.kind === "f64" || result.kind === "u32");
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
// The checks below read two named results, so they only run if both were produced.
if (analysed === 0) {
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
check("a released handle is refused by both stages (C6)", (() => {
  motor.release(stagedHandle);
  let post = false;
  let analysis2 = false;
  try {
    motor.post(stagedHandle, posts[0]);
  } catch {
    post = true;
  }
  try {
    motor.analysis(stagedHandle, analyses[0]);
  } catch {
    analysis2 = true;
  }
  return post && analysis2;
})());
}
}
