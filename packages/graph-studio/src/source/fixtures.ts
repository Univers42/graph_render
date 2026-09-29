/**
 * The fixtures the engine ships, as a picker.
 *
 * A list of paths, not a directory scan: the browser cannot read the repository,
 * and `scripts/studio.sh` copies `fixtures/` into `app/public/fixtures/`. These
 * are the files under the repository's `fixtures/` that carry nodes and edges —
 * the two that do not (`analysis/negative-weight.json`, `adversarial-ids.json`)
 * are edge-case notes rather than graphs, so they are not offered and the list
 * does not pretend otherwise.
 */

export const FIXTURES: readonly string[] = [
  "dag/chain.json",
  "dag/cyclic.json",
  "dag/diamond.json",
  "dag/disconnected.json",
  "dag/multi-span.json",
  "dag/wide-layer.json",
  "force/clustered.json",
  "force/disconnected.json",
  "force/grid.json",
  "force/single-node.json",
  "force/tree.json",
  "hierarchy/cyclic.json",
  "hierarchy/forest.json",
  "hierarchy/tree-balanced.json",
  "hierarchy/tree-degenerate.json",
  "post/hairball.json",
  "post/long-span.json",
  "post/obstacles.json",
  "post/parallel-edges.json",
  "analysis/disconnected.json",
  "analysis/star.json",
  "analysis/two-cliques.json",
  "analysis/weighted.json",
  "scale/n220.json",
];
