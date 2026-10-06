/**
 * `prelude.wgsl.ts` — the uniform every stage of the charge pass reads, plus two constants.
 *
 * It is a string and not a module because WGSL has no include: each kernel file composes
 * `PRELUDE_WGSL` with its own bindings and entry points, so the uniform's layout is written
 * once and every stage compiles against the same one. A layout written twice drifts.
 *
 * ## The layout, and why every field is 4 bytes
 *
 * A uniform struct in WGSL lives in the uniform address space, where a struct's size rounds
 * up to a multiple of 16. Sixteen `u32`/`f32` members is exactly 64 bytes and needs no padding
 * anywhere — the one layout in this pass a reader can check by counting. Nothing here is an
 * `f64`, and nothing is a `vec2`: the whole point of the arm is that the arithmetic is `f32`,
 * and an `f64` in a uniform would be a silent `f64` on the device.
 *
 * | field | | read by |
 * |---|---|---|
 * | `n` | node count | every stage's bounds guard |
 * | `side` | the transform's `P` | `fft`, `read`, the deposit's stencil |
 * | `bits` | `log2(side)`, the bit-reversal width | `fft` |
 * | `cells` | cells per axis the deposit may touch | `deposit`, `read` |
 * | `scale` | `scaleFor(n)`, the fixed-point scale | `deposit` |
 * | `blocks` | `⌈n/256⌉`, the bounds fold's block count | `fold_extent` |
 * | `live` | lines this pass transforms; the rest are zero | `fft` |
 * | `mode` | 0 rows, 1 rows × gain, 2 columns | `fft` |
 * | `inverse` | 1 conjugates the twiddle | `fft` |
 * | `fault` | which `--break` fault, 0 for none | every stage |
 * | `h`, `origin_x`, `origin_y` | the frame, from the fixture header | `deposit`, `read` |
 * | `charge` | `params.charge · alpha`, `alpha` at 1 | `read` |
 * | `inv_scale` | `1/scale`, exact: `scale` is a power of two | `widen_density` |
 *
 * `live`, `mode` and `inverse` are per-pass, which is why the four transforms get four
 * uniform buffers of this layout rather than sharing one: the CPU's `Fft::forward` and
 * `Fft::inverse` take a different `live` count for each of their two passes
 * (`fft/pass.rs:112-125`), and a per-pass uniform is the honest way to carry that.
 */

/** The uniform block, shared verbatim by every stage. */
export const PRELUDE_WGSL = `
struct Frame {
  n: u32,
  side: u32,
  bits: u32,
  cells: u32,
  scale: u32,
  blocks: u32,
  live: u32,
  mode: u32,
  inverse: u32,
  fault: u32,
  pad0: u32,
  h: f32,
  origin_x: f32,
  origin_y: f32,
  charge: f32,
  inv_scale: f32,
};

@group(0) @binding(0) var<uniform> frame: Frame;

// WGSL has no f32::MAX constant. The frame's own EMPTY box is this pair (frame.rs:27-30), and
// writing it as a bit pattern cannot drift from the value.
const F32_MAX: f32 = bitcast<f32>(0x7f7fffffu);
const F32_MIN: f32 = bitcast<f32>(0xff7fffffu);
`;

/** The uniform's size in bytes: sixteen 4-byte members, and 64 is a multiple of 16. */
export const FRAME_BYTES = 64;

/**
 * The positions, as `vec2<f32>`.
 *
 * One 8-byte read per node rather than two, because the deposit and the bounds fold both want
 * `x` and `y` of the same node together and one binding keeps the index arithmetic to one
 * term.
 *
 * Caveat: the host narrows these from the fixture's `f64` with `Math.fround` once, on upload.
 * That is the only narrowing of a position in the whole pass, so `boundsExact` compares the
 * arm's `f32` positions against the host's `f32` positions and not against `f64` ones.
 */
export const NODES_WGSL = `
@group(0) @binding(1) var<storage, read> nodes: array<vec2<f32>>;
`;

/**
 * The fault selectors, as the numbers the uniform carries.
 *
 * They are a `switch`-free set of `==` comparisons because WGSL has no enum in the uniform
 * address space and a `const` per fault keeps the kernels' comparisons named. `0` is none, so
 * an arm run without `--break` takes no branch at all.
 */
export const FAULTS = {
  /** `--break butterfly`: the twiddle index of the `hi` element is swapped. */
  BUTTERFLY: 1,
  /** `--break deposit`: every node's cell moves one step, weights unchanged. */
  DEPOSIT: 2,
  /** `--break bounds`: the extent fold skips its last block. */
  BOUNDS: 3,
  /** `--break weight`: one quantum is added to the fourth weight. */
  WEIGHT: 4,
} as const;