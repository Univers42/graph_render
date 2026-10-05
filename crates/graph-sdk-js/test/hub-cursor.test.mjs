import assert from "node:assert/strict";
import { test } from "node:test";
import { GraphMotorError } from "../src/errors.ts";
import { MAX_SEQ, formatCursor, parseCursor, InvalidCursorError } from "../src/hub/cursor.ts";

// The plan's Task 1 list plus the extra cases its own §"the only guard against a silently
// skipped change" and §12 condition 14 need. The ORDER is the contract: `negctl-hub-sdk-cursor`
// in `scripts/orch/rows/hub-sdk.rows` greps `not ok 8 - hub_cursor_accepts_the_ceiling_and_
// refuses_one_past_it`, and node's tap reporter numbers tests from 1 in declaration order.

test("hub_cursor_parses_a_microsecond_epoch", () => {
  assert.deepEqual(parseCursor("1789000000000000.118"), { epoch: 1789000000000000, seq: 118 });
  assert.equal(formatCursor({ epoch: 1789000000000000, seq: 118 }), "1789000000000000.118");
});

test("hub_cursor_max_seq_is_two_to_the_53_minus_one", () => {
  assert.equal(MAX_SEQ, 9007199254740991);
  assert.equal(Number.isSafeInteger(MAX_SEQ), true);
});

test("hub_cursor_round_trips_through_format", () => {
  for (const text of ["0.0", "1.0", "1789000000000000.118", "9007199254740991.9007199254740990"]) {
    assert.equal(formatCursor(parseCursor(text)), text, text);
  }
});

test("hub_cursor_accepts_the_origin", () => {
  assert.deepEqual(parseCursor("0.0"), { epoch: 0, seq: 0 });
});

test("hub_cursor_is_a_graph_motor_error", () => {
  // `src/hub/call.ts` and every existing caller catch `GraphMotorError`; a cursor refusal that
  // escaped that catch would be the one throw this package does not name.
  assert.throws(() => parseCursor("nope"), (e) => e instanceof InvalidCursorError && e instanceof GraphMotorError);
  assert.equal(new InvalidCursorError("x").name, "InvalidCursor");
});

test("hub_cursor_refuses_a_negative_half", () => {
  for (const bad of ["-1.2", "1.-2", "-0.0"]) {
    assert.throws(() => parseCursor(bad), InvalidCursorError, bad);
  }
});

test("hub_cursor_refuses_a_half_one_past_the_ceiling", () => {
  // `Number("9007199254740992")` is exactly representable, so only the `isSafeInteger` guard
  // refuses this; a reader that dropped that guard would hand back a 2^53 epoch.
  assert.throws(() => parseCursor("9007199254740992.0"), InvalidCursorError);
  assert.throws(() => parseCursor("0.9007199254740992"), InvalidCursorError);
});

test("hub_cursor_accepts_the_ceiling_and_refuses_one_past_it", () => {
  assert.deepEqual(parseCursor("9007199254740991.0"), { epoch: MAX_SEQ, seq: 0 });
  assert.throws(() => parseCursor("9007199254740992.1"), InvalidCursorError);
  assert.throws(() => parseCursor("0.9007199254740992"), InvalidCursorError);
});

test("hub_cursor_refuses_three_halves_and_a_bare_seq", () => {
  assert.throws(() => parseCursor("1.2.3"), InvalidCursorError);
  assert.throws(() => parseCursor("5"), InvalidCursorError);
});

test("hub_cursor_refuses_a_non_decimal_half", () => {
  for (const bad of ["1.", ".1", "01.1", "+1.2", " 1.2", "1 .2", "1e3.2", "1.2 "]) {
    assert.throws(() => parseCursor(bad), InvalidCursorError, bad);
  }
});

test("hub_cursor_refuses_an_empty_text", () => {
  assert.throws(() => parseCursor(""), InvalidCursorError);
});

test("hub_cursor_names_the_text_it_refused_without_a_key", () => {
  // The refusal carries the cursor text, which is a position, never a key: §5.1 keys never
  // reach an error message, and a cursor is not one.
  assert.throws(() => parseCursor("1.2.3"), /1\.2\.3/);
});
