// The golden spiral both stress arms start from, in one place.
//
// graph-core's seeder is `f64 libm::cos/sin` (`crates/graph-core/src/layout/force/
// barnes_hut/seed.rs:17-18`); this is V8's `Math.cos/sin`. They are not asserted
// bit-identical anywhere, because nothing in this repo can observe both at once — there
// is no `graph-cli` subcommand that dumps graph-core's seeds.
//
// Ponytail: a 1-ulp disagreement between libm's and V8's `cos`/`sin` puts one seed in a
// different place. Direction: 112 chaotic ticks amplify it, so it surfaces as a shifted
// stress correlation rather than as an error. Escape hatch: this arm computes no metric —
// `crates/graph-cli/src/stress/metric.rs` correlates both arms' positions once — so a
// seed gap is a claim the correlation cannot make, and the witness below is the honest
// bound: perturbing the angle by one ulp must move at least one seed, which is what makes
// the gap load-bearing rather than cosmetic.

export const GOLDEN_ANGLE = 2.399963229728653;
export const SEED_RADIUS = 12;

/** One node's seed on the spiral, for index `i`. Split out so the witness below can
 *  perturb the angle without duplicating the formula. */
function spiralPoint(i, angle) {
  const r = SEED_RADIUS * Math.sqrt(i + 1);
  return { x: Math.cos(i * angle) * r, y: Math.sin(i * angle) * r };
}

/** The `n` seed positions, index `i` at `(x[i], y[i])`. */
export function goldenSpiral(n, angle = GOLDEN_ANGLE) {
  const x = new Array(n);
  const y = new Array(n);
  for (let i = 0; i < n; i += 1) {
    const point = spiralPoint(i, angle);
    x[i] = point.x;
    y[i] = point.y;
  }
  return { x, y };
}

/**
 * Whether moving the angle by one ulp moves a seed: the witness that the seed is
 * load-bearing (see the note above). Over `n` indices the answer is `false` only if the
 * whole spiral is insensitive, which would mean the gap above cannot matter.
 */
export function seedMovesWithAngle(n) {
  const perturbed = goldenSpiral(n, nextUp(GOLDEN_ANGLE));
  const base = goldenSpiral(n);
  return base.x.some((x, i) => x !== perturbed.x[i] || base.y[i] !== perturbed.y[i]);
}

/** The next f64 above `value` — one ulp, the smallest gap a transcendental can differ by. */
function nextUp(value) {
  const buffer = new ArrayBuffer(8);
  new DataView(buffer).setFloat64(0, value);
  const view = new DataView(buffer);
  for (let byte = 7; byte >= 0; byte -= 1) {
    if (view.getUint8(byte) === 0xff) {
      view.setUint8(byte, 0x00);
      continue;
    }
    view.setUint8(byte, view.getUint8(byte) + 1);
    break;
  }
  return view.getFloat64(0);
}