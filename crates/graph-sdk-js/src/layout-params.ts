// One layout's published parameters, and the run buffer built from them
// (`docs/decisions/layout-params.md`): `gm_layout_params`'s framed body decoded, and the
// positional little-endian `f64` buffer `gm_run` reads.
//
// **The motor is the only authority on range.** This module checks the two things it can
// check without a round trip — a key the schema does not publish, and a value that is not
// a number — and sends everything else as sent. A value out of range comes back as
// `ParamOutOfRange` from the motor itself, because a second range check here would be a
// second rule to keep in step with the schema.

/** Width of one value in a run's parameter buffer: a little-endian `f64`. */
export const VALUE_BYTES = 8;

const decoder = new TextDecoder("utf-8", { fatal: true });

/** What a parameter's value means. Wire tags `0`, `1`, `2`; the motor refuses an `int`
 *  that is not whole and a `bool` that is not `0`/`1`. */
export type LayoutParamKind = "int" | "float" | "bool";

/** One parameter a layout publishes. The fields are the generated
 * `crates/graph-contract/generated/layout-params.d.ts` ones. */
export interface LayoutParamSpec {
  /** The key {@link Motor.run} takes it under: the layout's own field name. */
  name: string;
  /** What the value means. */
  kind: LayoutParamKind;
  /** Inclusive lower bound. A value below it is refused by the motor, never clamped. */
  min: number;
  /** Inclusive upper bound, refused the same way. */
  max: number;
  /** The value a run with no `params` takes. */
  default: number;
  /** The increment a control should offer. Never applied to a value. */
  step: number;
  /** One line, for a label and a tooltip. */
  doc: string;
}

const KINDS: readonly LayoutParamKind[] = ["int", "float", "bool"];

/** The schema in a `gm_layout_params` body, decoded. Throws on a body this SDK does not
 *  understand rather than answering `[]`: a layout that publishes nothing is a four-byte
 *  body, and anything shorter than that, or a tag no version publishes, is a mismatch a
 *  caller needs to hear about. */
export function decodeLayoutParams(bytes: Uint8Array): LayoutParamSpec[] {
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  if (bytes.byteLength < 4) throw new RangeError(`layout parameter schema is ${bytes.byteLength} bytes, too short to be one`);
  const count = view.getUint32(0, true);
  const specs: LayoutParamSpec[] = [];
  let at = 4;
  for (let i = 0; i < count; i += 1) {
    const name = readString(view, bytes, at);
    at = name.next;
    const tag = view.getUint8(at);
    const kind = KINDS[tag];
    if (kind === undefined) throw new RangeError(`parameter ${name.text}: kind tag ${tag} is not one of 0, 1, 2`);
    at += 1;
    const min = view.getFloat64(at, true);
    const max = view.getFloat64(at + 8, true);
    const fallback = view.getFloat64(at + 16, true);
    const step = view.getFloat64(at + 24, true);
    at += 32;
    const doc = readString(view, bytes, at);
    at = doc.next;
    specs.push({ name: name.text, kind, min, max, default: fallback, step, doc: doc.text });
  }
  return specs;
}

/** The run buffer for `values`: one little-endian `f64` per published parameter, in schema
 *  order — the order the schema publishes, never the order the caller wrote them in.
 *
 *  A name the schema does not publish is a `RangeError`, never a silent drop: a caller
 *  that misspelled `niter` would otherwise get a drawing that used the default and no
 *  word about it. A published name the caller left out takes its `default`, which is
 *  what leaving it out means.
 *
 *  Written through a `DataView` because a `Float64Array` over a `gm_alloc` pointer
 *  throws `RangeError` (the allocator hands out 4-aligned blocks — see
 *  `crates/graph-wasm/src/alloc.rs`), the same reason `force.ts` writes its own buffer
 *  this way. */
export function encodeLayoutParams(specs: readonly LayoutParamSpec[], values: Readonly<Record<string, number | boolean>>): Uint8Array {
  const byName = new Map(specs.map((spec) => [spec.name, spec]));
  for (const name of Object.keys(values)) {
    if (!byName.has(name)) {
      throw new RangeError(`"${name}" is not a parameter this layout publishes (${specs.map((s) => s.name).join(", ") || "it publishes none"})`);
    }
  }
  const bytes = new Uint8Array(specs.length * VALUE_BYTES);
  const view = new DataView(bytes.buffer);
  specs.forEach((spec, i) => {
    const given = values[spec.name];
    const value = given === undefined ? spec.default : given;
    if (typeof value === "boolean") {
      view.setFloat64(i * VALUE_BYTES, value ? 1 : 0, true);
      return;
    }
    if (typeof value !== "number" || !Number.isFinite(value)) {
      throw new TypeError(`${spec.name} is ${describe(value)}, not a finite number`);
    }
    view.setFloat64(i * VALUE_BYTES, value, true);
  });
  return bytes;
}

function describe(value: unknown): string {
  return typeof value === "number" ? `${value}` : `${typeof value} ${String(value)}`;
}

function readString(view: DataView, bytes: Uint8Array, at: number): { text: string; next: number } {
  if (at + 4 > bytes.byteLength) throw new RangeError("layout parameter schema ended inside a length");
  const len = view.getUint32(at, true);
  const from = at + 4;
  if (from + len > bytes.byteLength) throw new RangeError("layout parameter schema ended inside a string");
  return { text: decoder.decode(bytes.subarray(from, from + len)), next: from + len };
}