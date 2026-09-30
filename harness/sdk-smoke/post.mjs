// Every registered POST capability over the staged fixture.

import {
  ColumnId,
  PostRefusedError,
} from "../../crates/graph-sdk-js/src/index.ts";
import { check, refusedWith, columnProblems } from "./lib.mjs";

export async function runPostSection(ctx) {
  const { motor, node } = ctx;
// --- POST and ANALYSIS -------------------------------------------------------------------
//
// The same discipline as the layout loop above: the capability and analysis lists come
// from the module's own registries (`Motor#posts` / `Motor#analyses`, i.e.
// `gm_post_count`/`gm_post_id` and `gm_analysis_count`/`gm_analysis_id`), never from names
// written here. A POST capability or an analysis registered after this script was written
// is therefore covered with no edit to this file — which is the only way these two loops
// can keep meaning "every one" as the registries grow.

// The fixture the POST and ANALYSIS loops run over: a five-node graph with a parallel
// edge, a self-loop and a hierarchy chain, so a routing pass meets an enclosed node, a
// style pass meets a loop and a fan, and depth has a tree to count. Built through the
// published `build`, never through a raw export.
const staged = JSON.stringify({
  version: 1,
  nodes: ["a", "b", "c", "d", "z"].map((id) => node(id, "record")),
  edges: [
    { id: "r0", source: "a", target: "b", kind: "relation", label: "", strength: 0.5, directed: false, record_id: null },
    { id: "r1", source: "b", target: "c", kind: "relation", label: "", strength: 0.5, directed: false, record_id: null },
    { id: "r2", source: "a", target: "b", kind: "relation", label: "", strength: 0.5, directed: false, record_id: null },
    { id: "loop", source: "a", target: "a", kind: "relation", label: "", strength: 0.5, directed: false, record_id: null },
    { id: "h0", source: "b", target: "d", kind: "hierarchy", label: "parent_of", strength: 0.5, directed: true, record_id: null },
  ],
});
const stagedHandle = motor.build(staged);
check("the POST/ANALYSIS fixture builds", motor.nodeCount(stagedHandle) === 5);

const posts = typeof motor.posts === "function" ? motor.posts() : [];
check("the SDK publishes the module's POST registry (C1)", posts.length > 0);
check("the POST registry names the two bundlers, the route and the four styles", (() => {
  const want = [
    "post.bundle.fdeb", "post.bundle.mingle", "post.route.grid",
    "post.style.straight", "post.style.orthogonal", "post.style.quadratic", "post.style.bezier",
  ];
  return want.every((id) => posts.includes(id)) && new Set(posts).size === posts.length;
})());

// The contract's own table, restated the way a consumer reads it: which edge geometry
// kind each named capability emits. A pass that emitted a different kind would be
// inconsistent with the contract even if it ran without refusing.
const POST_EDGE_KIND = new Map([
  ["post.bundle.fdeb", "Polyline"],
  ["post.bundle.mingle", "Polyline"],
  ["post.route.grid", "Polyline"],
  ["post.style.straight", "Line"],
  ["post.style.orthogonal", "Polyline"],
  ["post.style.quadratic", "Curve"],
  ["post.style.bezier", "Curve"],
]);

motor.layout(stagedHandle, "layout.grid");
const gridNodeX = Array.from(motor.column(stagedHandle, ColumnId.NodeX));
let postRan = 0;
for (const postId of posts) {
  motor.layout(stagedHandle, "layout.grid");
  // As with the analyses: a refusal is a failure to report, not a reason to abort the
  // loop, so a broken pass does not hide the state of the other six.
  let result;
  try {
    result = motor.post(stagedHandle, postId);
  } catch (error) {
    check(`${postId}: the published SDK runs the pass`, false);
    process.stdout.write(`#   ${String(error)}\n`);
    continue;
  }
  postRan += 1;
  process.stdout.write(`# ${postId}: ${result.nodeKind} nodes / ${result.edgeKind} edges\n`);
  check(`${postId}: it reports the edge kind the contract names`, result.edgeKind === POST_EDGE_KIND.get(postId));
  check(`${postId}: it leaves the node kind alone`, result.nodeKind === "Point");
  check(`${postId}: the node positions are the layout's, unmoved`, (() => {
    const now = Array.from(motor.column(stagedHandle, ColumnId.NodeX));
    return now.length === gridNodeX.length && now.every((v, i) => Object.is(v, gridNodeX[i]));
  })());
  const problems = columnProblems(motor, stagedHandle, postId, result);
  check(`${postId}: its columns match the contract's presence table`, problems.length === 0);
  for (const problem of problems) process.stdout.write(`#   ${problem}\n`);
  check(
    `${postId}: its JSON face carries the fixture's nodes`,
    JSON.parse(motor.toJSON(stagedHandle)).nodes.id.length === 5,
  );
  check(`${postId}: every edge got a row when its kind stores one`, (() => {
    const offsets = motor.column(stagedHandle, ColumnId.EdgeOffsets);
    if (offsets === null) return result.edgeKind === "Line";
    const points = motor.column(stagedHandle, ColumnId.EdgePts);
    return offsets.length === 6 && points.length === 2 * offsets[offsets.length - 1];
  })());
}
check("every registered POST capability ran through the published SDK", postRan === posts.length && postRan > 0);

// A post pass needs a layout run: refused with a typed error, not a silent no-op, and
// never a drawing over nothing.
{
  const bare = motor.build(staged);
  let refused;
  try {
    motor.post(bare, posts[0]);
  } catch (error) {
    refused = error;
  }
  check("a post pass with no layout run yet is refused", refused instanceof PostRefusedError);
  check("the refusal names NoGeometryYet", refused?.codeName === "NoGeometryYet");
  check("an unknown post id is refused before the ABI is asked to run it", await refusedWith(PostRefusedError, () => motor.post(bare, "post.style.none")));
  check("an unknown post id in the registry listing is not silently accepted", !posts.includes("post.style.none"));
  motor.release(bare);
}
  ctx.staged = staged;
  ctx.stagedHandle = stagedHandle;
  ctx.posts = posts;
}
