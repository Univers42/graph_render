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

/// Hash every requested stage over every seed and print `<stage> <seed> <sha256>`, then
/// run the C20 comparison. Returns the arm's exit code: 0 ran (and compared), 1 the
/// comparison diverged, 2 refused.
///
/// Stage order is the caller's, seed order ascending, so two invocations over the same
/// module and the same seeds print byte-identical text.
export function runHash(abi, seeds, stages) {
  if (stages.length === 0) refuse("hash needs at least one stage");
  const digestsByStage = new Map(); // stage -> [digest, ...] by seed, for the C20 check
  const lines = [];
  for (const stage of stages) {
    // A shim-backed stage keeps its frozen hasher. Any other stage must name a layout,
    // analysis or POST capability the module itself registered, each resolved through its
    // registry (C1): a new registry row joins the gate with no change to this file, and a
    // misspelt stage is refused rather than silently hashed as something else.
    const bytesOf = abi.bytesFor(stage);
    const digests = [];
    for (let seed = 0; seed < seeds; seed += 1) {
      const digest = createHash("sha256").update(bytesOf(seed)).digest("hex");
      digests.push(digest);
      lines.push(`${stage} ${seed} ${digest}\n`);
    }
    digestsByStage.set(stage, digests);
  }
  process.stdout.write(lines.join(""));
  return c20(digestsByStage, seeds, stages.length);
}

/// C20: when both halves of the pair were asked for in the same invocation, this is the
/// actual enforced proof that the real ABI (`TRANSPORT_STAGE`) reaches the same bytes as
/// the retained hash-gate shim (`SHIM_LAYOUT`) — not just two printed digest lists a human
/// would otherwise have to compare by eye. Exits 1 on the first divergence found.
///
/// Both names come from `lib.mjs`'s `C20_PAIR`, which is the same pair `abi.mjs` keys its
/// stage table off, so the two sides of the comparison cannot be renamed apart (m37/m38).
/// A half-named pair is **not** left as a silent skip: see `c20Plan`'s two callers below.
function c20(digestsByStage, seeds, stageCount) {
  const plan = c20Plan([...digestsByStage.keys()]);
  if (plan.state === "neither") return 0;
  if (plan.state === "half") return c20Half(plan, stageCount);
  const reference = digestsByStage.get(SHIM_LAYOUT);
  const transport = digestsByStage.get(TRANSPORT_STAGE);
  for (let seed = 0; seed < seeds; seed += 1) {
    if (reference[seed] !== transport[seed]) {
      process.stderr.write(
        `wasm-run: C20 FAIL — ${TRANSPORT_STAGE} diverges from ${SHIM_LAYOUT} at seed ${seed} ` +
          `(${SHIM_LAYOUT} ${reference[seed]}, ${TRANSPORT_STAGE} ${transport[seed]})\n`,
      );
      return 1;
    }
  }
  process.stderr.write(
    `wasm-run: C20 ok — ${TRANSPORT_STAGE} == ${SHIM_LAYOUT} on all ${seeds} seeds\n`,
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
