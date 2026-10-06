/**
 * `read.wgsl.ts` — the CIC read of the field at each node, the pass's last stage.
 *
 * This is `mesh.rs:276-288` (`Mesh::read`) and `charge.rs:29-46` (`Interpolate`) as one
 * kernel. The CPU walks the nodes in the collide grid's order and reads each from the stencil
 * its deposit stored; the GPU walks them in dense order and recomputes the stencil from the
 * same `f32` positions and the same frame uniform the deposit used. Both therefore read the
 * *same* four cells with the *same* four weights — the recomputation is not a second stencil,
 * it is the identical expression `cell`/`split` evaluated once per stage instead of stored
 * between them.
 *
 * **The read does not follow the deposit fault, and that is the control.** `--break deposit`
 * moves every node's charge one cell along x and leaves the weights alone, so the field is the
 * true field of a density shifted one cell right. A read that shifted with it would sample the
 * shifted field at the shifted position and recover almost exactly what it would have read
 * anyway — the fault would be invisible, and a control that cannot go red proves nothing. A read
 * that stays at the node's own position samples a field one cell off, which is an O(1) error in
 * the increment and is what the comparator is there to see. The two stages agree on the stencil
 * in a clean run, which is the property the fault tests.
 *
 * **The four samples in the CPU's fixed order `(at, at+1, at+P, at+P+1)`**, zipped with
 * `weights(fx, fy)` in the order `(x,y), (x+1,y), (x,y+1), (x+1,y+1)` (`mesh.rs:280-282`,
 * `deposit.rs:186-193`). Four `f32` adds of four `f32` products, in that order and no other:
 * floating-point addition is not associative, so a reordered read is a different kernel that
 * happens to agree to a few ULP. The order is written out as four separate statements rather
 * than a loop over a weight array so that no compiler can turn it into a reassociated sum.
 *
 * **Both components are read.** The density is a complex column and the CPU accumulates
 * `.re` and `.im` separately (`mesh.rs:284-285`), with the imaginary part +0 everywhere
 * upstream because the deposit only ever adds to `.re` (`deposit.rs:173`). Reading only `.re`
 * would be a shortcut that the imaginary column's exact zero currently hides, and it is not
 * one this pass is allowed to take.
 *
 * The scale is `params.charge · alpha` (`charge.rs:44`), applied after the read rather than
 * folded into a weight — the fixture's deltas are at `alpha = 1` (`fixtures/gpu/README.md:118`),
 * so `charge` alone, and the host puts the product in the uniform's `charge` field.
 */

import { NODES_WGSL, PRELUDE_WGSL } from "./prelude.wgsl.ts";

/** The WGSL for the per-node field read. */
export const READ_WGSL = `${PRELUDE_WGSL}
${NODES_WGSL}
@group(0) @binding(2) var<storage, read> field: array<vec2<f32>>;
@group(0) @binding(3) var<storage, read_write> delta: array<vec2<f32>>;

// The lower cell of scaled coordinate u on one axis, inside 0..cells-1. Identical to the
// deposit's cell(), and it must stay identical: both stages read the same four cells or the
// stencil weights do not match the stencil that was deposited.
fn cell(u: f32) -> u32 {
  let hi = f32(frame.cells - 2u);
  return u32(clamp(u, 0.0, hi));
}

// The field at node k, scaled by charge * alpha, as the velocity increment. The four samples
// are in the CPU's order and are not reassociated: (at, at+1, at+side, at+side+1) against
// weights((x,y), (x+1,y), (x,y+1), (x+1,y+1)).
@compute @workgroup_size(256)
fn read_field(@builtin(global_invocation_id) id: vec3<u32>) {
  let k = id.x;
  if (k >= frame.n) {
    return;
  }
  let p = nodes[k];
  if (p.x != p.x || p.y != p.y) {
    return;
  }
  let ux = (p.x - frame.origin_x) / frame.h;
  let uy = (p.y - frame.origin_y) / frame.h;
  let cx = cell(ux);
  let cy = cell(uy);
  let fx = clamp(ux - f32(cx), 0.0, 1.0);
  let fy = clamp(uy - f32(cy), 0.0, 1.0);
  var at = cy * frame.side + cx;
  let w0 = (1.0 - fx) * (1.0 - fy);
  let w1 = fx * (1.0 - fy);
  let w2 = (1.0 - fx) * fy;
  let w3 = fx * fy;
  var e = field[at] * w0;
  e = e + field[at + 1u] * w1;
  e = e + field[at + frame.side] * w2;
  e = e + field[at + frame.side + 1u] * w3;
  delta[k] = e * frame.charge;
}
`;