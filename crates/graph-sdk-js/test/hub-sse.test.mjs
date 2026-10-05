import assert from "node:assert/strict";
import { test } from "node:test";
import { sseFrames } from "../src/hub/sse.ts";

// `ReadableStream` is not one of the eight globals `eslint.config.js:68-78` gives a `.mjs`
// file, so it is reached through `globalThis`; the constructor itself is node 22's own.
function streamOf(chunks) {
  const encoder = new TextEncoder();
  return new globalThis.ReadableStream({
    start(controller) {
      for (const chunk of chunks) controller.enqueue(encoder.encode(chunk));
      controller.close();
    },
  });
}

async function framesOf(chunks) {
  const out = [];
  for await (const frame of sseFrames(streamOf(chunks))) out.push(frame);
  return out;
}

test("hub_sse_reads_one_change_frame", async () => {
  const frames = await framesOf(['event: change\nid: 1.5\ndata: {"seq":7}\n\n']);
  assert.deepEqual(frames, [{ event: "change", id: "1.5", data: '{"seq":7}' }]);
});

test("hub_sse_reads_a_frame_split_across_three_chunks", async () => {
  // Mid-line on the first cut and mid-block on the second: the two cases a chunk-at-a-time
  // reader gets wrong in opposite directions.
  const frames = await framesOf(["event: ch", "ange\nid: 1.5\ndata: {\"seq\"", ":7}\n"]);
  assert.deepEqual(frames, [{ event: "change", id: "1.5", data: '{"seq":7}' }]);
});

test("hub_sse_reads_a_block_split_before_its_blank_line", async () => {
  const frames = await framesOf(["event: change\nid: 1.5\n", 'data: {"seq":7}', "\n\n"]);
  assert.deepEqual(frames, [{ event: "change", id: "1.5", data: '{"seq":7}' }]);
});

test("hub_sse_reads_crlf_line_endings", async () => {
  const frames = await framesOf(["event: change\r\nid: 1.5\r\ndata: {}\r\n\r\n"]);
  assert.deepEqual(frames, [{ event: "change", id: "1.5", data: "{}" }]);
});

test("hub_sse_splits_a_crlf_block_across_two_chunks", async () => {
  const frames = await framesOf(["event: change\r\nid: 1.5\r\n", "data: {}\r\n\r\n"]);
  assert.deepEqual(frames, [{ event: "change", id: "1.5", data: "{}" }]);
});

test("hub_sse_ignores_a_heartbeat_comment", async () => {
  // The hub writes `: keep-alive` on an idle stream (§5.3); a reader that turned one into a
  // frame would hand `subscribe` an empty change.
  assert.deepEqual(await framesOf([": keep-alive\n\n"]), []);
  assert.deepEqual(await framesOf([": keep-alive\n\nevent: busy\nid: 1.9\ndata: {}\n\n"]), [
    { event: "busy", id: "1.9", data: "{}" },
  ]);
});

test("hub_sse_joins_two_data_lines_with_a_newline", async () => {
  const frames = await framesOf(["event: change\ndata: one\ndata: two\n\n"]);
  assert.deepEqual(frames, [{ event: "change", id: undefined, data: "one\ntwo" }]);
});

test("hub_sse_defaults_a_block_with_no_event_to_message", async () => {
  const frames = await framesOf(["data: {}\n\n"]);
  assert.equal(frames[0].event, "message");
});

test("hub_sse_yields_no_id_when_the_block_has_none", async () => {
  const frames = await framesOf(["event: busy\ndata: {}\n\n"]);
  assert.equal(frames[0].id, undefined);
});

test("hub_sse_reads_two_frames_from_one_chunk", async () => {
  const frames = await framesOf(["event: busy\nid: 1.8\ndata: {}\n\nevent: change\nid: 1.9\ndata: {}\n\n"]);
  assert.deepEqual(frames.map((f) => [f.event, f.id]), [
    ["busy", "1.8"],
    ["change", "1.9"],
  ]);
});

test("hub_sse_flushes_a_trailing_block_with_no_blank_line", async () => {
  // A stream cut between the last line and its blank line still carried that notice.
  const frames = await framesOf(["event: change\nid: 1.5\ndata: {}\n"]);
  assert.deepEqual(frames, [{ event: "change", id: "1.5", data: "{}" }]);
});

test("hub_sse_honours_an_id_with_no_space_after_the_colon", async () => {
  const frames = await framesOf(["event: change\nid:1.5\ndata: {}\n\n"]);
  assert.equal(frames[0].id, "1.5");
});

test("hub_sse_ignores_a_field_it_does_not_know", async () => {
  const frames = await framesOf(["event: change\nretry: 3000\nid: 1.5\ndata: {}\n\n"]);
  assert.deepEqual(frames, [{ event: "change", id: "1.5", data: "{}" }]);
});

test("hub_sse_keeps_an_event_with_no_data_line", async () => {
  // §5.3 promises `data:` for `event: change` and does not promise one for `resync` or
  // `busy`. A reader that required a body would drop the one frame that ends a dead cursor.
  const frames = await framesOf(["event: resync\n\n"]);
  assert.deepEqual(frames, [{ event: "resync", id: undefined, data: "" }]);
});

test("hub_sse_reads_a_multi_byte_character_split_across_two_chunks", async () => {
  // `TextDecoder` in streaming mode is what makes this work; a `Buffer.toString()` per chunk
  // would turn the split into two replacement characters.
  const bytes = new TextEncoder().encode('event: change\nid: 1.5\ndata: "é"\n\n');
  const at = bytes.indexOf(0xc3);
  const stream = new globalThis.ReadableStream({
    start(controller) {
      controller.enqueue(bytes.slice(0, at + 1));
      controller.enqueue(bytes.slice(at + 1));
      controller.close();
    },
  });
  const out = [];
  for await (const frame of sseFrames(stream)) out.push(frame);
  assert.deepEqual(out, [{ event: "change", id: "1.5", data: '"é"' }]);
});

test("hub_sse_reads_an_empty_stream_as_no_frames", async () => {
  assert.deepEqual(await framesOf([]), []);
});
