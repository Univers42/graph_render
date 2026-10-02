//! `_disk_positions` (`SciGraphs/core/scigraphs_core/mesh/layouts/hierarchical.py:90-111`)
//! and the two numpy behaviours it leans on. `count` points over a disk of `radius`, on
//! evenly spaced concentric rings filled in order, ring `k` holding a share proportional
//! to its circumference.

/// `_disk_positions(count, radius)` (`hierarchical.py:90-111`), whole.
pub(crate) fn disk(count: usize, radius: f64) -> Vec<(f64, f64)> {
    if count == 1 {
        return vec![(0.0, 0.0)];
    }
    let rings = ring_count(count);
    let mut points = Vec::with_capacity(count);
    for (k, &n) in ring_take(count, rings).iter().enumerate() {
        if n > 0 {
            points.extend(ring_points(k, n, radius, rings));
        }
    }
    points
}

/// `rings = max(1, int(round(np.sqrt(count / np.pi))))` (`hierarchical.py:97`). The
/// `int(round(...))` is Python's banker's rounding, so it goes through [`half_to_even`]
/// like the `take` below; no count ever reaches an exact half here — that would need
/// `count / pi == (m + 1/2)^2` and pi is irrational — so the rule is never what decides
/// this line, but routing it through the same function keeps the one rounding rule in the
/// module.
pub(crate) fn ring_count(count: usize) -> usize {
    let raw = f64::sqrt(count as f64 / core::f64::consts::PI);
    half_to_even(raw).max(1.0) as usize
}

/// `share = np.arange(rings) + 0.5`, `take = np.round(share / share.sum() * count)` as
/// ints, and both fix-up loops (`hierarchical.py:98-103`) folded in.
///
/// The loops are load-bearing: `take` does not sum to `count` in general (at `count = 23`
/// it comes out as `[3, 8, 13]`, one too many), so dropping either one changes the drawing.
pub(crate) fn ring_take(count: usize, rings: usize) -> Vec<i64> {
    let total = share_total(rings);
    let mut take: Vec<i64> = (0..rings)
        .map(|k| half_to_even((k as f64 + 0.5) / total * count as f64) as i64)
        .collect();
    let mut sum: i64 = take.iter().sum();
    while sum > count as i64 {
        let at = argmax(&take);
        take[at] -= 1;
        sum -= 1;
    }
    while sum < count as i64 {
        let last = take.len() - 1;
        take[last] += 1;
        sum += 1;
    }
    take
}

/// `share.sum()` for `share = np.arange(rings) + 0.5`: the running total of `0.5, 1.5, ...`
/// is a multiple of `0.5` bounded by `rings^2 / 2`, which is exactly representable while
/// `rings < 2^26` — and `rings` is `sqrt(count / pi)`, so `count` would have to exceed
/// `2^52` first, long past any `u32` topology. The sum is therefore exact, and *no*
/// summation order could change its bits; the loop below is the reference's order anyway
/// (D3), so this is a note, not a guard.
fn share_total(rings: usize) -> f64 {
    let mut total = 0.0_f64;
    for k in 0..rings {
        total += k as f64 + 0.5;
    }
    total
}

/// `np.argmax` (`hierarchical.py:101`): the **first** maximum, so a tie between two rings
/// takes the lower ring index — the element numpy returns. `Iterator::max` would take the
/// last, and the fix-up would then debits a different ring.
fn argmax(take: &[i64]) -> usize {
    let mut best = take[0];
    let mut at = 0;
    for (i, &value) in take.iter().enumerate() {
        if value > best {
            best = value;
            at = i;
        }
    }
    at
}

/// `np.round` / Python's `round` on a float: to nearest, **ties to even**
/// (`hierarchical.py:97` and `:99`). `f64::round` is ties *away from zero* and disagrees at
/// every exact half — at `count = 10` the reference's `take` is `[2, 8]` here and `[3, 7]`
/// through `f64::round` plus its own fix-up loop, which is a different drawing. So the
/// rule is written out instead of delegated, even though `libm::floor` is one of the four
/// transcendentals D1 allows.
///
/// `floor` is the reference's own `np.round` semantics for the values this module rounds:
/// every one is non-negative, so `floor` is the truncation the sign-free rule agrees with.
pub(crate) fn half_to_even(value: f64) -> f64 {
    let down = libm::floor(value);
    let fraction = value - down;
    // Exactly one half is the only case the even rule decides, and there the even of
    // `down`/`down + 1` wins. Every other value — and a NaN, which compares false to both
    // arms and so falls through to `down` exactly as `np.round(NaN)` gives NaN — rounds to
    // the nearer side.
    let nearer = if fraction == 0.5 {
        libm::fmod(down, 2.0) != 0.0
    } else {
        fraction > 0.5
    };
    if nearer { down + 1.0 } else { down }
}

/// One ring of the disk (`hierarchical.py:105-110`): its `n` points at
/// `angle = arange(n) * (2 * pi / n)`, on `radius * (k + 0.5) / rings`. The step is divided
/// once *before* the multiply, as the reference writes it — `(2*pi/n)` is a single value
/// numpy then scales, and hoisting the division into the loop would not be the same bytes.
fn ring_points(k: usize, n: i64, radius: f64, rings: usize) -> Vec<(f64, f64)> {
    let r = radius * (k as f64 + 0.5) / rings as f64;
    let step = 2.0 * core::f64::consts::PI / n as f64;
    (0..n)
        .map(|j| {
            let angle = j as f64 * step;
            (r * libm::cos(angle), r * libm::sin(angle))
        })
        .collect()
}
