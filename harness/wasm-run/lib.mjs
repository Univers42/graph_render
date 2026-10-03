// The wasm arm's refusals, the two stage names every comparison derives from, and the
// pure decisions `harness/wasm-run.mjs`'s modes make. Split out by the house's 300-line
// limit (the review's m40) and importable on its own, so `harness/wasm-run.test.mjs`
// pins a refusal or a seed count without a wasm artifact in hand.

/// "Could not run": the arm's exit 2. Thrown rather than written straight to stderr, so
/// every refusal is one value the entry point turns into `wasm-run: <message>` and exit 2
/// — the 0/1/2 table `harness/wasm-run.mjs`'s header promises. Nothing else in this
/// directory decides an exit code.
export class Refusal extends Error {}

/// Refuse with `message`: exit 2, "could not run", never a stack trace.
export function refuse(message) {
  throw new Refusal(message);
}

/// The layout the transport stage runs, and the one the retained `gm_layout_grid` shim
/// hashes. Both are this layout, named once: the shim predates this phase's real ABI and
/// is frozen so Phase 2/3's already-green gate keeps hashing the bytes it always hashed,
/// so this stage stays shim-backed while every other registered layout is hashed through
/// the real ABI. `abiSnapshotBytes` drives this id through `gm_run`/`gm_snapshot_bytes`,
/// and `hashgate`'s native side names it in `hashgate/stages.rs` as `LAYOUT`.
export const SHIM_LAYOUT = "layout.grid";

/// The real-ABI-over-the-shim-layout stage: the C20 measurement the ledger reads. Named
/// once, beside `SHIM_LAYOUT`, because `hashgate/stages.rs` names it `TRANSPORT` and the
/// C20 check below has to name the same two stages the table above hashed.
export const TRANSPORT_STAGE = "transport.wasm.columnar";

/// The C20 pair, one derivation: both halves come from the constants the stage table is
/// built from, so the comparison's two sides can never be keyed off different names
/// (the review's m37/m38 — a `digestsByStage.get("layout.grid")` literal beside a
/// `SHIM_LAYOUT`-keyed transport is a rename away from a silent skip).
export const C20_PAIR = Object.freeze([SHIM_LAYOUT, TRANSPORT_STAGE]);

/// The seed count `hash` mode will sweep, or a refusal.
///
/// **Zero is refused, not accepted** (the review's m37, read as `U4`). `"0"` satisfies
/// `/^[0-9]+$/`, so the old arm printed no digest line, ran the C20 loop zero iterations
/// and wrote "C20 ok … on all 0 seeds" at exit 0: an exit code reading as evidence for a
/// comparison that never ran. This is the same refusal `hashgate.rs`'s
/// `refuse_a_vacuous_control` makes for the same reason, and `hashgate/transport.rs`'s
/// `agree_with_shim` answers `0 seeds` with "a tally over nothing proves nothing".
export function parseSeedCount(text) {
  const seeds = Number.parseInt(text ?? "", 10);
  if (!/^[0-9]+$/.test(text ?? "") || seeds > 0xffffffff) refuse(`bad seed count ${text}`);
  if (seeds === 0) {
    refuse("0 seeds: a tally over nothing proves nothing — run `hash` with a seed count of 1 or more");
  }
  return seeds;
}

/// What the C20 check can do with the stages this invocation asked for:
/// `pair` (both halves, compare them), `neither` (the caller asked for neither, there is
/// nothing to prove and nothing to say), or `half` (one half asked for alone).
///
/// A pure function of the requested names, so the decision is unit-testable without a
/// module and the loud paths are one table away.
export function c20Plan(requested) {
  const present = C20_PAIR.filter((stage) => requested.includes(stage));
  const missing = C20_PAIR.filter((stage) => !requested.includes(stage));
  if (missing.length === 0) return { state: "pair", present, missing };
  if (present.length === 0) return { state: "neither", present, missing };
  return { state: "half", present, missing };
}

/// The stderr note for a one-stage invocation that named one half of the C20 pair.
///
/// Ponytail: this is a note, not a refusal, and that is the whole of its weakness — a
/// deliberate "can this one stage be hashed?" probe is the same shape as a gate run that
/// forgot its other half, and the arm cannot tell them apart. `hashgate/tests/stages/
/// arm.rs` drives the first (`assert_hashable(&wasm, TRANSPORT)`) and would go red on the
/// second, so the *multi-stage* half of the rule is the one that refuses; this note covers
/// what is left, and it says on stderr that the proof did not run rather than passing
/// silently.
export function c20HalfNote(present, missing) {
  return `C20 NOT checked — ${present[0]} was hashed without ${missing[0]}, so nothing proved the real ABI reaches the shim's bytes. Name both ${C20_PAIR[0]} and ${C20_PAIR[1]} in one invocation for the comparison.`;
}

/// The refusal for a multi-stage invocation that named one half of the C20 pair. A
/// multi-stage run is a gate run: `hashgate.rs` drives this arm with its own whole stage
/// list, so a list missing one half of the C20 pair is a mistake that would otherwise
/// print a transport tally that is not a proof, at exit 0 and with no message.
export function refuseAHalfPair(present, missing, stageCount) {
  refuse(
    `${stageCount} stages requested but the C20 pair is half of that: ${present[0]} without ${missing[0]}. ` +
      `The comparison needs both ${C20_PAIR[0]} and ${C20_PAIR[1]} in one invocation, ` +
      "or neither — a run that hashes one half alone proves nothing about the real ABI.",
  );
}
