/** What the console remembers of what was typed, and where in it the input stands. */

const LIMIT = 100;

export interface History {
  readonly lines: readonly string[];
  /** The line the input is showing; `lines.length` is the reader's own text. */
  readonly at: number;
}

export interface Step {
  readonly history: History;
  /** The line to show, or `null` to empty the input again. */
  readonly line: string | null;
}

export const EMPTY_HISTORY: History = { lines: [], at: 0 };

export function pushLine(history: History, line: string): History {
  const trimmed = line.trim();
  const last = history.lines[history.lines.length - 1];
  if (trimmed === "" || trimmed === last) return history;
  const lines = [...history.lines, trimmed].slice(-LIMIT);
  return { lines, at: lines.length };
}

export function stepBack(history: History): Step {
  if (history.at === 0) return { history, line: history.lines[0] ?? null };
  const at = history.at - 1;
  return { history: { ...history, at }, line: history.lines[at] ?? null };
}

export function stepForward(history: History): Step {
  if (history.at >= history.lines.length) return { history, line: null };
  const at = history.at + 1;
  return { history: { ...history, at }, line: history.lines[at] ?? null };
}
