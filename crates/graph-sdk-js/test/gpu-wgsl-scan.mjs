// The node-only WGSL scan, shared by the test and its negative control.
//
// It is the one place that knows what a kernel source must look like, so the control can
// rewrite a source in memory and ask the same question the test asks. Two rules, both from
// `docs/decisions/gpu-g1.md` condition 8 and the software arm's limits
// (`docs/measurements/gpu-adapter.md:120-121`):
//
// 1. Every entry point is `@workgroup_size(256)`. The software arm caps invocations per
//    workgroup at 256, so 256 is the largest size every kernel may carry on both arms; the
//    hardware arm's 1024 buys nothing because no stage in the charge pass is that wide.
// 2. No kernel mentions `f16`. The software arm exposes no `shader-f16`
//    (`gpu-adapter.md:130-131`), so an f16 path would be a hardware-only path by
//    construction — and `src/gpu/types.ts` declares no f16 type at all to take one.
//
// The second rule is a substring test, not a token test, and deliberately so: it is the same
// test the `no-f16` gate row runs over `src/gpu`, so a kernel and its row cannot disagree
// about what "mentions f16" means.

/** One kernel source under the scan. */
export const WG = 256;

/**
 * Every problem in `sources`, as human-readable strings; empty when there is none.
 *
 * @param {readonly {name: string, code: string}[]} sources the kernels to scan
 * @returns {string[]} one line per problem, naming the kernel and the text
 */
export function scanWgsl(sources) {
  const problems = [];
  for (const source of sources) {
    problems.push(...widthProblems(source), ...precisionProblems(source));
  }
  return problems;
}

/** The `@compute` entry points of `source` that are not `@workgroup_size(256)`. */
function widthProblems(source) {
  const found = source.code.match(/@compute\b/g) ?? [];
  const problems = [];
  for (const at of source.code.matchAll(/@compute\b/g)) {
    const tail = source.code.slice(at.index, at.index + 80);
    if (!/^@compute\s+@workgroup_size\(\s*256\s*\)/.test(tail)) {
      problems.push(`${source.name}: an @compute entry point is not @workgroup_size(${WG})`);
    }
  }
  if (found.length === 0 && source.name !== "shared") {
    problems.push(`${source.name}: no @compute entry point at all`);
  }
  return problems;
}

/** The `f16` mentions in `source`, if any. */
function precisionProblems(source) {
  return source.code.toLowerCase().includes("f16")
    ? [`${source.name}: the source mentions f16`]
    : [];
}