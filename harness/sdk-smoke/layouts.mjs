// Every registered layout, run through the published SDK.

import { check, boundsOf, columnProblems } from "./lib.mjs";

export async function runLayoutSection(ctx) {
  const { motor, handle } = ctx;
// Every layout the module registered, read once through the SDK's own view of the
// registry (C1). A hard-coded name here would make this file cover exactly one layout
// forever: p3's four would arrive, the smoke test would keep passing, and none of them
// would ever have been run through the published SDK.
const registered = typeof motor.layouts === "function" ? motor.layouts() : [];
check("the SDK publishes the module's layout registry (C1)", registered.length > 0);
check(
  "the registry names layout.grid and repeats no id",
  registered.includes("layout.grid") && new Set(registered).size === registered.length,
);

let ran = 0;
for (const layoutId of registered) {
  const result = motor.layout(handle, layoutId);
  ran += 1;
  const bounds = boundsOf(motor, handle);
  process.stdout.write(
    `# ${layoutId}: ${result.nodeCount} nodes, ${result.nodeKind} nodes / ${result.edgeKind} edges, ` +
      `bounds x[${bounds.minX}, ${bounds.maxX}] y[${bounds.minY}, ${bounds.maxY}]\n`,
  );
  check(`${layoutId}: every node is placed`, result.nodeCount === 2);
  check(
    `${layoutId}: its bounds are finite and not a single point`,
    [bounds.minX, bounds.maxX, bounds.minY, bounds.maxY].every(Number.isFinite) &&
      (bounds.maxX > bounds.minX || bounds.maxY > bounds.minY),
  );
  const problems = columnProblems(motor, handle, layoutId, result);
  check(`${layoutId}: its columns match the contract's presence table`, problems.length === 0);
  for (const problem of problems) process.stdout.write(`#   ${problem}\n`);
  check(
    `${layoutId}: its JSON face carries the same nodes`,
    JSON.parse(motor.toJSON(handle)).nodes.id.length === 2,
  );
}
check("every registered layout ran through the published SDK", ran === registered.length && ran > 0);
  ctx.ran = ran;
}
