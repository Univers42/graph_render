/**
 * `fft.wgsl.ts` — the 2D transform as four line passes, one workgroup per line.
 *
 * ## The pass structure this mirrors, exactly
 *
 * The CPU writes the 2D transform as two range passes per direction, and each pass's `live`
 * count differs (`fft/pass.rs:107-126`). Four dispatches, in this order:
 *
 * | # | direction | `live` | read | inverse | in → out |
 * |---|---|---:|---|---|---|
 * | 1 | rows | `cells` | `Rows` | no | `a` → `b` |
 * | 2 | columns | `side` | `Columns` | no | `b` → `a` |
 * | 3 | rows | `side` | `RowsTimes(gain)` | yes | `a` → `b` |
 * | 4 | columns | `cells` | `Columns` | yes | `b` → `a` |
 *
 * Two things are easy to get wrong and are pinned here rather than left to the reader:
 *
 * - **`live` is not `P` for every pass.** Pass 1 skips input rows `cells` on, which the
 *   deposit never stored; pass 4 skips *output* rows `cells` on, which no node reads
 *   (`fft/pass.rs:8-11`). A row at or past `live` is written as `+0`, which is what the CPU's
 *   `dst.fill(C::default())` does and is exact: a line of zeroes transforms to a line of
 *   zeroes.
 * - **The kernel multiply is fused into pass 3's load**, not a fifth dispatch
 *   (`fft/pass.rs:52-56`): `dst[k] = src[k] · gain[k]`, elementwise over the whole spectrum,
 *   once, in `f64` on the CPU and in `f32` here. The `1/P²` is *already* in `gain` — the
 *   fixture's `spectrum_*` is the kernel pre-scaled by `1/P²` (`fixtures/gpu/README.md:108`),
 *   so nothing divides by `P²` again.
 *
 * **Nothing normalises.** Both CPU transforms are unnormalised and the `1/P²` lives in the
 * kernel (`fft.rs:10-11`, `kernel.rs:66-68`), so the uploaded spectrum already carries it. A
 * `1/P` here would double-count it.
 *
 * ## The line transform
 *
 * `Plan::line` (`fft.rs:134-158`) in order: the bit-reversal swaps, then the `half == 1` stage
 * as a plain sum and difference with no twiddle, then `half = 2, 4, … < side` with the twiddle
 * run `twiddle[half..2·half]`. Each of the 256 invocations strides the line's `i, i+256, …`
 * and every access is `i < P`-guarded, which is the price of one workgroup size on both arms:
 * at `P = 128` two thirds of the invocations idle (`plan :994-996`).
 *
 * **Bit reversal is integer arithmetic only** — `reverseBits` on the index and a shift — so it
 * is exact in `f32` and on both arms. The enumeration is the CPU's: `i` over the whole line,
 * `j = reverseBits(i) >> (32 − bits)`, swap only when `i < j`, so the same pairs in the same
 * count. Filtering to `i < P/2` instead would be a different permutation and is not done.
 *
 * **Twiddles come from the uploaded table, never from `sin`/`cos`.** The fixture carries
 * `Plan::forward` whole (`fft.rs:128-130`) and WGSL's transcendentals are good to 2⁻¹¹
 * absolute, which is the same order as the deposit quantum this pass is bounded by
 * (`gpu-force-tier.md:53-55`). The table is narrowed to `f32` **once, on upload**, so the
 * transform's error is one narrowing and not `log₂P` of them.
 *
 * The inverse conjugates rather than carrying a second table, exactly as `Plan::new` builds it
 * (`fft.rs:106-112`): same `w`, negated imaginary part.
 *
 * Caveat: **the fold order inside a stage is fixed but the pair order is not observable.** A
 * stage's butterflies are pairwise disjoint, so no summation order is chosen at all — each
 * output sample is one `+` and one `·` on distinct inputs. What *is* fixed, and what makes the
 * arm repeatable, is that no value is ever summed over a different set than the CPU summed it
 * over: the same `half`, the same twiddle index, the same cell order. Failing input: a
 * different `P` than the fixture's, where `bits` and the stage ladder would not match the
 * uploaded twiddle table's construction. Direction: a wrong transform by O(1), caught by the
 * butterfly control. Escape hatch: `inv_scale` and the table's narrowing are the only two
 * places a value is transformed between `f64` and `f32`, and both are single multiplies.
 */

import { PRELUDE_WGSL } from "./prelude.wgsl.ts";

/** The WGSL for the four line passes. */
export const FFT_WGSL = `${PRELUDE_WGSL}
@group(0) @binding(1) var<storage, read> gain: array<vec2<f32>>;
@group(0) @binding(2) var<storage, read> src: array<vec2<f32>>;
@group(0) @binding(3) var<storage, read_write> dst: array<vec2<f32>>;
@group(0) @binding(4) var<storage, read> twiddle: array<vec2<f32>>;

// The largest side a line can have, so the workgroup array can be sized at compile time:
// 1024 complex samples is 8 KB, inside both arms' workgroup storage (gpu-adapter.md:85, :120).
const MAX_SIDE: u32 = 1024u;

var<workgroup> line: array<vec2<f32>, 1024>;

// One complex sample's two words, the multiplication fft.rs:60-65 defines: re = a.re*b.re -
// a.im*b.im, im = a.re*b.im + a.im*b.re. Plain products, no fused form, because the CPU's are
// plain and a contraction would make the two arms differ by a rounding step per butterfly.
fn cmul(a: vec2<f32>, b: vec2<f32>) -> vec2<f32> {
  return vec2<f32>(a.x * b.x - a.y * b.y, a.x * b.y + a.y * b.x);
}

// Pass 3's fused kernel multiply, and pass 1 and 4's plain copy, are both this load. mode 2 is
// the transpose fused into the read (fft/pass.rs:57-61): src[r + i*side] is input column r.
fn load(r: u32, i: u32) -> vec2<f32> {
  if (frame.mode == 2u) {
    return src[r + i * frame.side];
  }
  let at = r * frame.side + i;
  if (frame.mode == 1u) {
    return cmul(src[at], gain[at]);
  }
  return src[at];
}

// The twiddle for stage \`half\` tap \`j\`, conjugated when this pass is the inverse. The table
// is Plan::forward whole, narrowed to f32 once on upload, indexed exactly as the CPU does at
// fft.rs:148.
fn twiddle_at(half: u32, j: u32) -> vec2<f32> {
  var tap = j;
  // --break butterfly flips the low bit of the hi element's twiddle index. The plan's control:
  // a broken butterfly is off by O(1) in the field, far above any f32 bound
  // (gpu-force-tier.md:100-101), which is what makes it the control the record's gate names.
  if (frame.fault == 1u && half > 1u) {
    tap = j ^ 1u;
  }
  let w = twiddle[half + tap];
  if (frame.inverse != 0u) {
    return vec2<f32>(w.x, -w.y);
  }
  return w;
}

// One line of side samples: load, bit-reverse, then the stages. workgroupBarrier() sits between
// phases and never inside an i-dependent branch, because a barrier in non-uniform control flow
// is undefined in WGSL — hence the guards below are \`if (i < side)\`, not early returns.
@compute @workgroup_size(256)
fn fft_line(@builtin(workgroup_id) group: vec3<u32>,
            @builtin(local_invocation_id) lane: vec3<u32>) {
  let r = group.x;
  let side = frame.side;
  let t = lane.x;
  // A line at or past live is +0 in and +0 out (fft/pass.rs:44-47). The deposit never stored
  // input rows cells.. and no node reads output rows cells.., so this is the CPU's own
  // pruning and not a shortcut.
  //
  // The whole row is written, and it has to be: t is the lane index, 0..255, so a single
  // 't < side' store covers a quarter of a 1024-side line and leaves the rest of the row holding
  // whatever the buffer held before — uninitialized bytes on the first run, the first run's
  // leftovers on the second. That is a non-determinism, not a stale value: the two runs of the
  // repeat check disagree, and the disagreement is in the delta, which is the only column the
  // transform writes. The stride below is the load loop's and the store loop's, and the row is
  // 'side' entries whichever of the three writes it.
  if (r >= frame.live) {
    for (var i = t; i < side; i = i + 256u) {
      dst[r * side + i] = vec2<f32>(0.0, 0.0);
    }
    return;
  }
  for (var i = t; i < side; i = i + 256u) {
    line[i] = load(r, i);
  }
  workgroupBarrier();
  // The bit-reversal swaps, the CPU's enumeration: i over the whole line, swap only when the
  // reversed index is the larger one, so the pairs are the CPU's pairs. fft.rs:86-90, :135-137.
  for (var i = t; i < side; i = i + 256u) {
    let j = reverseBits(i) >> (32u - frame.bits);
    if (j > i) {
      let a = line[i];
      let b = line[j];
      line[i] = b;
      line[j] = a;
    }
  }
  workgroupBarrier();
  // half == 1: the first stage's twiddle is 1, so it is a plain sum and difference
  // (fft.rs:138-140). No multiply, and so no fault can touch it.
  for (var k = t; k * 2u + 1u < side; k = k + 256u) {
    let u = line[k * 2u];
    let v = line[k * 2u + 1u];
    line[k * 2u] = u + v;
    line[k * 2u + 1u] = u - v;
  }
  workgroupBarrier();
  // half = 2, 4, ... < side. A stage's butterflies are pairwise disjoint, so each output is one
  // add and one multiply on distinct inputs and no summation order is chosen. fft.rs:141-157.
  var half = 2u;
  loop {
    if (half >= side) {
      break;
    }
    let span = half * 2u;
    for (var k = t; k * 2u < side; k = k + 256u) {
      let j = k % half;
      let lo = (k / half) * span + j;
      let u = line[lo];
      let v = line[lo + half];
      let tv = cmul(v, twiddle_at(half, j));
      line[lo] = u + tv;
      line[lo + half] = u - tv;
    }
    workgroupBarrier();
    half = half * 2u;
  }
  for (var i = t; i < side; i = i + 256u) {
    dst[r * side + i] = line[i];
  }
}
`;

/** The largest side the kernel's workgroup array is sized for, as a number. */
export const MAX_SIDE = 1024;

/**
 * The workgroup storage the kernel actually allocates: `MAX_SIDE` complex samples at 8 bytes.
 *
 * `adapter.ts` checks this against `maxComputeWorkgroupStorageSize`, not `P · 8`. The array is
 * fixed-size in WGSL — a `var<workgroup>` cannot be sized from a uniform — so this fixed 8 KB
 * is what every compile allocates whatever `P` is, and a check against `P · 8` would pass a
 * device that then fails to compile. 8 KB is inside both arms' 65 536 and 32 768
 * (`gpu-adapter.md:85`, `:120`).
 */
export const FFT_WORKGROUP_BYTES = MAX_SIDE * 8;