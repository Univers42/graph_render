/**
 * One `.gmfx`, the file `graph-cli emit-gpu-fixtures` writes: the particle mesh's own
 * per-pass velocity increments at one state, as the GPU arm loads them.
 *
 * The format is specified in `fixtures/gpu/README.md`, and that README is normative: every
 * offset below quotes it. This reader adds nothing to it and derives nothing the file does
 * not already say — the header's `n`, `m` and `P` are the only lengths, and each section
 * follows from them.
 *
 * Two rules that are the reader's own and not the format's:
 *
 * - **The loader is version-strict.** A `format major` it does not know is a throw, never a
 *   best-effort read: a reader that accepted a future major would read a longer header as
 *   this one's fields and produce silently wrong numbers. A newer *minor* is read, because
 *   the minor only ever adds sections after the ones named here.
 * - **Every `Float64Array` view is checked for 8-byte alignment before it is built.** A
 *   `u32` section of odd length shifts every `f64` after it by four bytes, and the values
 *   are then silently wrong — the failure mode `docs/contract/binary-layout.md` is about.
 */

/** One `.gmfx`, as the header's words plus typed-array views over its own bytes. */
export interface Fixture {
  /** Node count `n`. */
  readonly n: number;
  /** Simple-edge count `m`, after graph-core's own collapse. */
  readonly m: number;
  /** The transform side `P`, a power of two in 128..=1024. */
  readonly side: number;
  /** `0` at the start state, `1` at the settled one. */
  readonly state: 0 | 1;
  /** The frame's rung; `h = 2^(step/4)`, read as a signed 32-bit word. */
  readonly step: number;
  /** The cell size in position units. */
  readonly h: number;
  /** The frame's left edge. */
  readonly originX: number;
  /** The frame's bottom edge. */
  readonly originY: number;
  /** Cells per axis. */
  readonly cells: number;
  /** The kernel's reach, in cells. */
  readonly reach: number;
  readonly edgeLo: Uint32Array;
  readonly edgeHi: Uint32Array;
  readonly edgeStrength: Float64Array;
  readonly posX: Float64Array;
  readonly posY: Float64Array;
  readonly twiddleRe: Float64Array;
  readonly twiddleIm: Float64Array;
  /** The kernel spectrum, already pre-scaled by `1/P²` — do not divide again. */
  readonly spectrumRe: Float64Array;
  readonly spectrumIm: Float64Array;
  /** The three passes' own increments from rest, per node in row order. */
  readonly delta: Readonly<Record<Pass, { x: Float64Array; y: Float64Array }>>;
}

/** Which of the mesh's three gathered passes a delta column is. */
export type Pass = "link" | "charge" | "collide";

/** The `format major` this reader knows. */
const MAJOR = 1;

/** The magic every `.gmfx` opens with. */
const MAGIC = 0x58464d47; // "GMFX" little-endian

/** Header length in bytes: ten `u32` words then three `f64`s. */
const HEADER_LEN = 64;

/** Reads one `.gmfx`. Throws on a wrong magic, an unknown major, or a truncated payload. */
export function loadFixture(bytes: ArrayBuffer): Fixture {
  const view = new DataView(bytes);
  if (bytes.byteLength < HEADER_LEN) {
    throw new Error(`gmfx: ${bytes.byteLength} bytes is shorter than the ${HEADER_LEN}-byte header`);
  }
  if (view.getUint32(0, true) !== MAGIC) {
    throw new Error("gmfx: the magic is not GMFX, so this is not a fixture");
  }
  const major = view.getUint32(4, true);
  if (major !== MAJOR) {
    throw new Error(`gmfx: format major ${major}, and this reader knows ${MAJOR}`);
  }
  const n = view.getUint32(12, true);
  const m = view.getUint32(16, true);
  const side = view.getUint32(20, true);
  // `state` is 0 or 1 by the format's own table (`fixtures/gpu/README.md:63`), and the narrow
  // type is built from the comparison rather than asserted, because this file is held to
  // `no-assert` (`scripts/orch/rows/gpu-g1b.rows`).
  const state: 0 | 1 = view.getUint32(24, true) === 1 ? 1 : 0;
  const step = view.getInt32(32, true);
  const h = view.getFloat64(40, true);
  const originX = view.getFloat64(48, true);
  const originY = view.getFloat64(56, true);
  let at = HEADER_LEN;
  const cells = view.getUint32(at, true);
  at += 4;
  const reach = view.getUint32(at, true);
  at += 4;
  const edgeLo = u32s(bytes, at, m, "edge_lo");
  at += 4 * m;
  const edgeHi = u32s(bytes, at, m, "edge_hi");
  at += 4 * m;
  const edgeStrength = f64s(bytes, at, m, "edge_strength");
  at += 8 * m;
  const posX = f64s(bytes, at, n, "pos_x");
  at += 8 * n;
  const posY = f64s(bytes, at, n, "pos_y");
  at += 8 * n;
  const twiddleRe = f64s(bytes, at, side, "twiddle_re");
  at += 8 * side;
  const twiddleIm = f64s(bytes, at, side, "twiddle_im");
  at += 8 * side;
  const spectrum = side * side;
  const spectrumRe = f64s(bytes, at, spectrum, "spectrum_re");
  at += 8 * spectrum;
  const spectrumIm = f64s(bytes, at, spectrum, "spectrum_im");
  at += 8 * spectrum;
  const delta = deltas(bytes, at, n);
  at += 48 * n;
  if (at !== bytes.byteLength) {
    throw new Error(`gmfx: the header says ${at} bytes and the file holds ${bytes.byteLength}`);
  }
  return { n, m, side, state, step, h, originX, originY, cells, reach, edgeLo, edgeHi, edgeStrength, posX, posY, twiddleRe, twiddleIm, spectrumRe, spectrumIm, delta };
}

/**
 * The Rust `scale_for`, exactly: `2^(31 - ceil(log2 n))`, and `undefined` for `n = 0`.
 *
 * **`**` and not `<<`**: JavaScript's `<<` is signed 32-bit, so `1 << 31` is `-2147483648`,
 * and the operator this replaces is exactly the signed one. `n = 1` is where a signed-shift
 * mistake is one bit from correct rather than visibly wrong, which is why
 * `the_two_scales_agree` includes it.
 */
export function scaleFor(n: number): number | undefined {
  if (n < 1) return undefined;
  return 2 ** (31 - (32 - Math.clz32(n)));
}

/**
 * The three delta column pairs, read at `at` in wire order: `link_x, link_y, charge_x,
 * charge_y, collide_x, collide_y`, each `n` words apart (`fixtures/gpu/README.md:90-92`).
 *
 * The object is built as a literal with all three keys rather than filled in a loop, because a
 * literal needs no type assertion and an empty-object-plus-assignment needs one — and this
 * file is held to `no-assert` (`scripts/orch/rows/gpu-g1b.rows`). The loop it replaces advanced
 * `at` by `16n` per pass; the sum is `48n`, which the caller's one `at` update carries.
 */
function deltas(bytes: ArrayBuffer, at: number, n: number): Record<Pass, { x: Float64Array; y: Float64Array }> {
  const pair = (pass: Pass, from: number): { x: Float64Array; y: Float64Array } => ({
    x: f64s(bytes, from, n, `delta_${pass}_x`),
    y: f64s(bytes, from + 8 * n, n, `delta_${pass}_y`),
  });
  return {
    link: pair("link", at),
    charge: pair("charge", at + 16 * n),
    collide: pair("collide", at + 32 * n),
  };
}

/** A `Uint32Array` view over `count` words at `at`, or a throw naming the section. */
function u32s(bytes: ArrayBuffer, at: number, count: number, name: string): Uint32Array {
  return view(bytes, at, count, name, Uint32Array, 4);
}

/** A `Float64Array` view over `count` words at `at`, or a throw naming the section. */
function f64s(bytes: ArrayBuffer, at: number, count: number, name: string): Float64Array {
  return view(bytes, at, count, name, Float64Array, 8);
}

/**
 * One typed-array view, with both checks the format makes necessary: the payload is long
 * enough for the section, and — for the `f64` case — the section starts 8-byte aligned. An
 * odd-length `u32` section ahead of it is what breaks that, and the values would then be
 * silently wrong rather than absent.
 */
function view<T extends Uint32Array | Float64Array>(
  bytes: ArrayBuffer,
  at: number,
  count: number,
  name: string,
  ctor: { new (buffer: ArrayBuffer, byteOffset: number, length: number): T },
  width: 4 | 8,
): T {
  const end = at + width * count;
  if (end > bytes.byteLength) {
    throw new Error(`gmfx: ${name} ends at ${end} and the file holds ${bytes.byteLength} bytes`);
  }
  if (at % width !== 0) {
    throw new Error(`gmfx: ${name} starts at byte ${at}, which is not ${width}-byte aligned`);
  }
  return new ctor(bytes, at, count);
}
