/**
 * The embed example's replay: `fixtures/embed/replay.jsonl`, one ingest-v1 batch per line, handed
 * to `<graph-studio>.applyDeltas` a line at a time. It is what `deploy/nav/embedreplay.py` drives:
 * the gate clicks the page's Replay button and reads `window.__embed.replay`, so what the rows
 * judge is the host API alone — no `studio`, no `view`, nothing behind the element.
 *
 * WHY each choice:
 *   - One `await` per line, never in parallel. `applyDeltas` is atomic per call and not coalesced
 *     across calls (`docs/contract/host-api.md` condition 8), so a host that fires six calls at
 *     once learns nothing about the order the batches mean; a refusal is then one answer among six.
 *   - A refusal is data, not a fault. It pushes the motor's own `name` and the replay goes on to
 *     the next line, which is what a host streaming a log does. The same name arrives as
 *     `graph-error` (`element.ts:140`); the rows check the two agree.
 *   - `state` carries the outcome, `applied` and `refused` the per-line answers: the fixed shape
 *     `deploy/nav/embedreplay.py` reads, so a page that reports nothing is a row that fails.
 */
import { type GraphStudioElement } from "../../packages/graph-studio/src/element.ts";

/** The file the Replay button streams, one batch per line. */
export const REPLAY_URL = "fixtures/embed/replay.jsonl";

/**
 * The one `state` that means the element has no verb to call. `host-api.md` condition 8 is
 * "hosts feature-test with `"applyDeltas" in el`", and `deltarows.py:30` is the precedent for what
 * the gate does when the verb is gone: NOT-RUN with that wording, never a silent pass.
 */
export const NO_VERB = "failed applyDeltas is not on the element";

/** What the page publishes on `window.__embed.replay`, and what the gate reads. */
export interface ReplayState {
  /** `idle`, `running`, `done`, `failed <message>` or {@link NO_VERB}. */
  state: string;
  /** Each answering batch's `applied`, in line order: the nodes that went in. */
  applied: number[];
  /** The motor's error `name` per refused line, in line order. */
  refused: string[];
}

/** A replay that has not been started. */
export function fresh(): ReplayState {
  return { state: "idle", applied: [], refused: [] };
}

/** The batches, one per non-blank line. A bad line is `JSON.parse`'s own throw, which fails the
 *  whole replay: a batch a host cannot read is not a batch it may skip. */
function batches(text: string): readonly unknown[] {
  const parsed: unknown[] = [];
  for (const line of text.split("\n")) {
    if (line.trim() === "") continue;
    const batch: unknown = JSON.parse(line);
    parsed.push(batch);
  }
  return parsed;
}

async function read(url: string): Promise<string> {
  const response = await fetch(url);
  if (!response.ok) throw new Error(`${url}: HTTP ${response.status}`);
  return response.text();
}

/** One line's answer, kept. A refusal names itself and the next line still runs. */
async function one(element: GraphStudioElement, batch: unknown, into: ReplayState): Promise<void> {
  try {
    const answer = await element.applyDeltas(batch);
    into.applied.push(answer.applied);
  } catch (error) {
    into.refused.push(error instanceof Error ? error.name : "a non-Error");
  }
}

/** Streams the file into the element, in order, and leaves the outcome in `into`. */
export async function replay(element: GraphStudioElement, into: ReplayState): Promise<void> {
  into.state = "running";
  if (!("applyDeltas" in element)) {
    into.state = NO_VERB;
    return;
  }
  try {
    for (const batch of batches(await read(REPLAY_URL))) await one(element, batch, into);
    into.state = "done";
  } catch (error) {
    into.state = `failed ${error instanceof Error ? error.message : "a non-Error"}`;
  }
}