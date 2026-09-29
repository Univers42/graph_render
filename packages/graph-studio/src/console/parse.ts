/**
 * The console's grammar: a command word, then values by position or as `name=value`.
 * Quotes keep spaces; a value that starts with a quote is never read as `name=value`.
 */
import type { Action, Args, RawArgs, Registry } from "../actions/registry.ts";

export class CommandRefusal extends Error {
  constructor(message: string) {
    super(message);
    this.name = "CommandRefusal";
  }
}

export interface Command {
  readonly id: string;
  readonly raw: RawArgs;
}

interface Word {
  readonly text: string;
  /** Started with a quote: a value, whatever it contains. */
  readonly quoted: boolean;
}

interface Scan {
  readonly words: Word[];
  text: string;
  open: boolean;
  quoted: boolean;
  quote: string;
}

function close(scan: Scan): void {
  if (scan.open) scan.words.push({ text: scan.text, quoted: scan.quoted });
  scan.text = "";
  scan.open = false;
  scan.quoted = false;
}

function inQuote(scan: Scan, text: string, at: number): number {
  const char = text[at] ?? "";
  if (char === scan.quote) {
    scan.quote = "";
    return at + 1;
  }
  const escaped = char === "\\" && scan.quote === '"' && at + 1 < text.length;
  scan.text += escaped ? (text[at + 1] ?? "") : char;
  return at + (escaped ? 2 : 1);
}

function outside(scan: Scan, char: string): void {
  if (/\s/.test(char)) {
    close(scan);
    return;
  }
  if (char === '"' || char === "'") {
    if (!scan.open) scan.quoted = true;
    scan.quote = char;
  } else {
    scan.text += char;
  }
  scan.open = true;
}

function scanWords(text: string): Word[] {
  const scan: Scan = { words: [], text: "", open: false, quoted: false, quote: "" };
  let at = 0;
  while (at < text.length) {
    if (scan.quote !== "") {
      at = inQuote(scan, text, at);
    } else {
      outside(scan, text[at] ?? "");
      at += 1;
    }
  }
  if (scan.quote !== "") throw new CommandRefusal(`a ${scan.quote} quote is never closed`);
  close(scan);
  return scan.words;
}

export function tokenise(text: string): string[] {
  return scanWords(text).map((word) => word.text);
}

function put(raw: Record<string, unknown>, name: string, value: string): void {
  if (name in raw) throw new CommandRefusal(`\`${name}\` is given twice`);
  raw[name] = value;
}

function readValues<State, Context>(action: Action<State, Context>, words: readonly Word[]): RawArgs {
  const raw: Record<string, unknown> = {};
  let position = 0;
  for (const word of words) {
    const split = word.quoted ? -1 : word.text.indexOf("=");
    if (split > 0) {
      put(raw, word.text.slice(0, split), word.text.slice(split + 1));
      continue;
    }
    const spec = action.params[position];
    if (spec === undefined) {
      const count = action.params.length;
      throw new CommandRefusal(`\`${action.alias}\` takes ${count} value${count === 1 ? "" : "s"}`);
    }
    put(raw, spec.name, word.text);
    position += 1;
  }
  return raw;
}

export function parseCommand<State, Context>(text: string, registry: Registry<State, Context>): Command {
  const [first, ...rest] = scanWords(text);
  if (first === undefined) throw new CommandRefusal("nothing to run");
  const action = registry.find(first.text);
  if (action === undefined) {
    throw new CommandRefusal(`\`${first.text}\` is not a command; nearest: ${registry.suggest(first.text).join(", ")}`);
  }
  return { id: action.id, raw: readValues(action, rest) };
}

function quoted(value: string): string {
  if (value !== "" && !/[\s"'=\\]/.test(value)) return value;
  return `"${value.replace(/[\\"]/g, (char) => `\\${char}`)}"`;
}

/** The line that, typed in the console, runs `action` with `args`. */
export function formatCommand<State, Context>(action: Action<State, Context>, args: Args): string {
  const values = action.params.map((spec) => quoted(String(args[spec.name] ?? "")));
  return [action.alias, ...values].join(" ");
}
