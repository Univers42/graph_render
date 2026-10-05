// `hash` mode: the per-seed digests, and the C20 comparison between the real ABI and the
// retained shim. Split out of `harness/wasm-run.mjs` by the house's 300-line limit.

import { createHash } from "node:crypto";
import {
  c20HalfNote,
  c20Plan,
  refuse,
  refuseAHalfPair,
  SHIM_LAYOUT,
  TRANSPORT_STAGE,
} from "./lib.mjs";

/// Hash every requested stage over every seed of `shard` and print `<stage> <seed> <sha256>`,
/// then run the C20 comparison. Returns the arm's exit code: 0 ran (and compared), 1 the
/// comparison diverged, 2 refused.
///
/// Stage order is the caller's, seed order ascending, so two invocations over the same
/// module and the same seeds print byte-identical text — and a shard's lines are a subset of
/// the whole arm's, in the same positions, so `graph-cli`'s `shard::merge` puts them back
/// where the un-sharded arm printed them.
///
/// The seeds are a **stride** (`i`, `i+K`, `i+2K`), not a slice, and that is this file's
/// half of the reason: a seed's cost grows with its node count, so a contiguous split would
/// leave the arm as slow as its heaviest shard. Same seeds, same stage order, fewer lines.
export function runHash(abi, seeds, stages, shard = WHOLE_SHARD) {
  if (stages.length === 0) refuse("hash needs at least one stage");
  const shardSeeds = [...seedStride(shard, seeds)];
  const digestsByStage = new Map(); // stage -> [digest, ...] over shardSeeds, for the C20 check
  const lines = [];
  for (const stage of stages) {
    // A shim-backed stage keeps its frozen hasher. Any other stage must name a layout,
    // analysis or POST capability the module itself registered, each resolved through its
    // registry (C1): a new registry row joins the gate with no change to this file, and a
    // misspelt stage is refused rather than silently hashed as something else.
    const bytesOf = abi.bytesFor(stage);
    const digests = [];
    for (const seed of shardSeeds) {
      const digest = createHash("sha256").update(bytesOf(seed)).digest("hex");
      digests.push(digest);
      lines.push(`${stage} ${seed} ${digest}\n`);
    }
    digestsByStage.set(stage, digests);
  }
  process.stdout.write(lines.join(""));
  return c20(digestsByStage, shardSeeds, stages.length, shard);
}

/// The whole run in one shard: the default, so a hand-run probe needs no `--shard`.
export const WHOLE_SHARD = Object.freeze({ index: 0, count: 1 });

/// `--shard i/K`, or a refusal naming the text.
///
/// The same four refusals `hashgate/shard.rs`'s `Shard::parse` makes, in the same words,
/// because the two must not drift: this side strides the seeds, that side places the lines,
/// and a shard one side accepts and the other cannot place is an arm with a hole in it.
/// `K == 0` is refused (a stride of zero never terminates), `i >= K` is refused (a shard of
/// a run that does not exist would print nothing and read as an agreement).
export function parseShard(text) {
  const bad = (why) => refuse(`bad shard ${JSON.stringify(text ?? null)}: ${why}`);
  const match = /^([0-9]+)\/([0-9]+)$/.exec(String(text ?? ""));
  if (!match) bad("expected i/K, like 0/4");
  const index = Number(match[1]);
  const count = Number(match[2]);
  if (count === 0) bad("K is 0, so there is no shard to run");
  if (index >= count) bad(`i is ${index}, so it is not one of the ${count} shards 0..${count}`);
  return { index, count };
}

/// The seeds `shard` runs out of `seeds`, ascending: `i`, `i+count`, `i+2*count`, …
export function* seedStride(shard, seeds) {
  for (let seed = shard.index; seed < seeds; seed += shard.count) yield seed;
}

/// C20: when both halves of the pair were asked for in the same invocation, this is the
/// actual enforced proof that the real ABI (`TRANSPORT_STAGE`) reaches the same bytes as
/// the retained hash-gate shim (`SHIM_LAYOUT`) — not just two printed digest lists a human
/// would otherwise have to compare by eye. Exits 1 on the first divergence found.
///
/// `shardSeeds` is the list this invocation actually ran, and the divergence is reported at
/// the seed **number** in it — the digests are indexed by position, so under a shard
/// reporting `position 1` would name seed `index+1` whenever `index` is not 0 and
/// `count > 1`: the real seed is `shardSeeds[position]`.
///
/// Both names come from `lib.mjs`'s `C20_PAIR`, which is the same pair `abi.mjs` keys its
/// stage table off, so the two sides of the comparison cannot be renamed apart (m37/m38).
/// A half-named pair is **not** left as a silent skip: see `c20Plan`'s two callers below.
function c20(digestsByStage, shardSeeds, stageCount, shard) {
  const plan = c20Plan([...digestsByStage.keys()]);
  if (plan.state === "neither") return 0;
  if (plan.state === "half") return c20Half(plan, stageCount);
  const reference = digestsByStage.get(SHIM_LAYOUT);
  const transport = digestsByStage.get(TRANSPORT_STAGE);
  for (let position = 0; position < shardSeeds.length; position += 1) {
    if (reference[position] !== transport[position]) {
      process.stderr.write(
        `wasm-run: C20 FAIL — ${TRANSPORT_STAGE} diverges from ${SHIM_LAYOUT} at seed ${shardSeeds[position]} ` +
          `(${SHIM_LAYOUT} ${reference[position]}, ${TRANSPORT_STAGE} ${transport[position]})\n`,
      );
      return 1;
    }
  }
  process.stderr.write(
    `wasm-run: C20 ok — ${TRANSPORT_STAGE} == ${SHIM_LAYOUT} on all ${shardSeeds.length} seeds (shard ${shard.index}/${shard.count})\n`,
  );
  return 0;
}

/// The half-named pair. A multi-stage invocation refuses (exit 2): `hashgate.rs` drives
/// this arm with its own whole stage list, so a list carrying one half alone is a gate run
/// whose C20 proof would silently not happen. A one-stage invocation is a deliberate
/// "can this stage be hashed?" probe — `hashgate/tests/stages/arm.rs` drives exactly that
/// shape — so it keeps exit 0 and says on stderr that nothing was proved.
function c20Half(plan, stageCount) {
  if (stageCount > 1) refuseAHalfPair(plan.present, plan.missing, stageCount);
  process.stderr.write(`wasm-run: ${c20HalfNote(plan.present, plan.missing)}\n`);
  return 0;
}
