// The service's failures as `GraphMotorError`s (`docs/contract/service-api.md` "Errors").
// The body `{"error": "<code>", "message": "<one line>"}` is keyed by `error`, which is the
// motor's own `Code` name where one applies, so `codeName` reads the same whether the wasm
// path or the service refused, and `code` is that name's wire value. The names only the
// service emits live here, not in `CODE_NAMES`: that list is pinned, value by value, to
// graph-wasm's `Code` (`crates/graph-wasm/src/errors/mirrors.rs`), and these have no wire value.
//
// The key never reaches an error: messages are built from the route (never the URL, never a
// header) and from one line of the server's message, with the key cut out if it appears.

import { CODE_NAMES, GraphMotorError } from "../errors.ts";

/** The `error` names the service sends when the motor never ran or is not the one refusing,
 *  as `server/graph-server/src/error.rs` spells them: 400, 401, 404, 406, 429, 500, 503.
 *  Append-only, like `CODE_NAMES`. */
export const SERVICE_ERROR_NAMES = ["BadRequest", "Unauthorized", "NotFound", "NotAcceptable", "Busy", "Internal", "Timeout"] as const;

/** Every `error` name the SDK types: the motor's `Code` names and the service's own. */
export type RemoteErrorName = (typeof CODE_NAMES)[number] | (typeof SERVICE_ERROR_NAMES)[number];

const KNOWN_NAMES: ReadonlySet<string> = new Set<string>([...CODE_NAMES, ...SERVICE_ERROR_NAMES]);

const isKnownName = (name: string | undefined): name is RemoteErrorName => name !== undefined && KNOWN_NAMES.has(name);

export interface RemoteErrorInit {
  readonly status: number;
  /** The body's `error` string, or `undefined` when there was no such body. */
  readonly error?: string | undefined;
  readonly retryAfter?: number | undefined;
}

/** A request the service refused (any status but 200), a response this SDK cannot read, or a
 *  request that never got a response (`status` 0). */
export class RemoteError extends GraphMotorError {
  /** The HTTP status, or 0 when no response arrived. */
  readonly status: number;
  /** The body's `error` (`"UnknownLayoutId"`, `"Unauthorized"`, ...), or `undefined` when
   *  there was no error body or it named something this SDK does not type. The message still
   *  quotes an unknown name. */
  override readonly codeName: RemoteErrorName | undefined;
  /** Whole seconds from `Retry-After` (a 429 sends 1), or `undefined` when there was none. */
  readonly retryAfter: number | undefined;

  constructor(message: string, init: RemoteErrorInit) {
    const index = init.error === undefined ? -1 : (CODE_NAMES as readonly string[]).indexOf(init.error);
    super(message, index < 0 ? undefined : index);
    this.status = init.status;
    this.codeName = isKnownName(init.error) ? init.error : undefined;
    this.retryAfter = init.retryAfter;
  }
}

const MESSAGE_CHARS = 200;

/** `text` with every occurrence of `key` replaced, cut to its first line and a bounded length. */
export function scrub(text: string, key: string | undefined): string {
  const clean = key === undefined ? text : text.split(key).join("[redacted]");
  return (clean.split("\n")[0] ?? "").slice(0, MESSAGE_CHARS);
}

/** Caveat: delta-seconds only. The HTTP-date form, which the service never sends, reads as
 *  `undefined` rather than a guessed delay; a caller then picks its own backoff. */
function retryAfterOf(response: Response): number | undefined {
  const raw = response.headers.get("retry-after")?.trim() ?? "";
  return /^\d+$/.test(raw) ? Number(raw) : undefined;
}

interface ErrorBody {
  readonly error: string;
  readonly message: string | undefined;
}

function errorBodyOf(text: string): ErrorBody | undefined {
  try {
    const body: unknown = JSON.parse(text);
    if (typeof body !== "object" || body === null) return undefined;
    const { error, message } = body as { error?: unknown; message?: unknown };
    if (typeof error !== "string") return undefined;
    return { error, message: typeof message === "string" ? message : undefined };
  } catch {
    return undefined;
  }
}

/** The typed error for a non-200 response. A body that is not the service's error shape (a
 *  proxy's HTML page, say) leaves `codeName` undefined and is not quoted. */
export async function refusalOf(response: Response, route: string, key: string | undefined): Promise<RemoteError> {
  const text = await response.text().catch(() => "");
  const body = errorBodyOf(text);
  const head = `${route}: HTTP ${response.status}`;
  const error = body === undefined ? undefined : scrub(body.error, key);
  const named = error === undefined ? head : `${head} ${error}`;
  const detail = body?.message === undefined ? "" : `: ${scrub(body.message, key)}`;
  return new RemoteError(`${named}${detail}`, {
    status: response.status,
    error,
    retryAfter: retryAfterOf(response),
  });
}

function describe(cause: unknown): string {
  if (!(cause instanceof Error)) return String(cause);
  const inner = cause.cause instanceof Error ? `: ${cause.cause.message}` : "";
  return `${cause.message}${inner}`;
}

/** No response at all: the network, DNS, a refused redirect, an aborted signal. The cause is
 *  described, not attached, so nothing the fetch implementation captured travels with it. */
export function unreachable(route: string, cause: unknown, key: string | undefined): RemoteError {
  return new RemoteError(`${route}: no response: ${scrub(describe(cause), key)}`, { status: 0 });
}

/** A 200 whose body this SDK cannot read as the face it asked for. */
export function unreadable(route: string, detail: string): RemoteError {
  return new RemoteError(`${route}: ${detail}`, { status: 200 });
}
