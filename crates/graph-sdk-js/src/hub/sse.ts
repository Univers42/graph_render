// The SSE frame reader for `GET /v1/workspaces/{ws}/events` (§5.3).
//
// `EventSource` is not used anywhere in this package and cannot be: it has no way to send
// `Authorization`, and every hub route is behind a key. So the frames come off a `fetch` body
// stream, parsed here. This is the wire's grammar, not an approximation of it: a leading `:`
// is a comment, `field: value` drops exactly one space, `data:` lines join with a newline and
// a block with no `event:` is `message` — the four rules `EventSource` would have applied.

/** One server-sent event: its type, the `Last-Event-ID` to resume from, and its raw data. */
export interface SseFrame {
  readonly event: string;
  readonly id: string | undefined;
  readonly data: string;
}

// The blank line that ends a block, in either spelling. No `g` flag, so `exec` always starts
// at the head of the buffer and keeps no `lastIndex` between calls.
const SEPARATOR = /\r?\n\r?\n/;
const LINE_END = /\r?\n/;

/** Yields one frame per block of `stream`, as the hub writes them.
 *
 * Chunks are decoded with a streaming `TextDecoder`, so a multi-byte character split across
 * two chunks is one character and not two replacements, and a block split anywhere — mid-line,
 * mid-block, between the two halves of a `\r\n\r\n` — is still one frame. A block the stream
 * ended in the middle of is emitted rather than dropped: the hub closes a stream on `resync`,
 * on `busy` and when a heartbeat write fails, and a notice that arrived just before the close
 * is a notice the subscriber has to see.
 */
export async function* sseFrames(stream: ReadableStream<Uint8Array>): AsyncGenerator<SseFrame> {
  const reader = stream.getReader();
  const decoder = new TextDecoder();
  let buffer = "";
  for (;;) {
    const chunk = await reader.read();
    buffer += chunk.done ? decoder.decode() : decoder.decode(chunk.value, { stream: true });
    for (;;) {
      const taken = takeBlock(buffer, chunk.done);
      if (taken === null) break;
      buffer = taken.rest;
      const frame = frameOf(taken.block);
      if (frame !== null) yield frame;
    }
    if (chunk.done) return;
  }
}

// The first complete block and the rest of the buffer, or `null` while the buffer holds no
// complete block. `flush` (the stream has ended) takes a trailing block with no blank line.
function takeBlock(buffer: string, flush: boolean): { block: string; rest: string } | null {
  const match = SEPARATOR.exec(buffer);
  if (match === null) return flush && buffer !== "" ? { block: buffer, rest: "" } : null;
  return { block: buffer.slice(0, match.index), rest: buffer.slice(match.index + match[0].length) };
}

// A block of nothing but comments and fields the reader does not know is not an event: that is
// the 15 s comment heartbeat (§5.3) and an idle stream is idle, not busy. A block that named an
// `event` is one even with no `data:` line, because §5.3 sends `resync` and `busy` without
// promising a body and dropping `resync` would strand the subscriber on a dead cursor.
function frameOf(block: string): SseFrame | null {
  let event = "message";
  let named = false;
  let id: string | undefined;
  const data: string[] = [];
  for (const line of block.split(LINE_END)) {
    if (line === "" || line.startsWith(":")) continue;
    const field = fieldOf(line);
    if (field.name === "event") {
      event = field.value;
      named = true;
    } else if (field.name === "id") id = field.value;
    else if (field.name === "data") data.push(field.value);
  }
  if (!named && data.length === 0) return null;
  return { event, id, data: data.join("\n") };
}

// `field: value`, or a bare `field` meaning an empty value. Exactly one leading space of the
// value is dropped, so `id:1.5` and `id: 1.5` are the same id and a data line that means to
// start with a space can send two.
function fieldOf(line: string): { name: string; value: string } {
  const colon = line.indexOf(":");
  if (colon < 0) return { name: line, value: "" };
  const value = line.slice(colon + 1);
  return { name: line.slice(0, colon), value: value.startsWith(" ") ? value.slice(1) : value };
}
