//! `tql2` — implicit-shift QL with eigenvector accumulation, split out of `dense_sym.rs`
//! to stay under the house line cap. Ported line-for-line from JAMA `tql2`
//! (`docs/decisions/eigensolver.md`), plus the ADR's 30-iteration cap (JAMA's own loop
//! has none) and the stable `(value, index)` sort replacing JAMA's selection sort (D5).

use super::Sym;

/// Symmetric tridiagonal QL algorithm with implicit shifts (JAMA `tql2`). Returns
/// whether any eigenvalue hit the 30-iteration cap the ADR adds (JAMA's loop has none).
pub(super) fn tql2(sym: &mut Sym) -> bool {
    let n = sym.n;
    if n == 0 {
        return false;
    }
    for i in 1..n {
        sym.e[i - 1] = sym.e[i];
    }
    sym.e[n - 1] = 0.0;
    let eps = f64::EPSILON;
    let mut f = 0.0_f64;
    let mut tst1 = 0.0_f64;
    let mut hit_cap = false;
    for l in 0..n {
        tst1 = tst1.max(sym.d[l].abs() + sym.e[l].abs());
        let thresh = eps * tst1;
        let m = tql2_find_m(sym, l, thresh);
        if m > l && !tql2_converge(sym, (l, m), &mut f, thresh) {
            hit_cap = true;
        }
        sym.d[l] += f;
        sym.e[l] = 0.0;
    }
    tql2_sort(sym);
    hit_cap
}

/// The smallest `m >= l` with a negligible subdiagonal, or `n` when none is found short
/// of the end.
fn tql2_find_m(sym: &Sym, l: usize, thresh: f64) -> usize {
    let mut m = l;
    while m < sym.n {
        if sym.e[m].abs() <= thresh {
            break;
        }
        m += 1;
    }
    m
}

/// Iterates the implicit-shift QL step until `e[l]` is negligible, capped at 30
/// iterations (the ADR's cap; JAMA's own loop has none). Returns `false` on cap-out.
fn tql2_converge(sym: &mut Sym, range: (usize, usize), f: &mut f64, thresh: f64) -> bool {
    let (l, m) = range;
    for _ in 0..30 {
        tql2_shift(sym, l, m, f);
        if sym.e[l].abs() <= thresh {
            return true;
        }
    }
    false
}

/// One implicit QL transformation step over columns `l..=m` (JAMA `tql2`'s do-while
/// body): the shift, then the plane-rotation sweep from `m-1` down to `l`.
fn tql2_shift(sym: &mut Sym, l: usize, m: usize, f: &mut f64) {
    let n = sym.n;
    let g = sym.d[l];
    let mut p = (sym.d[l + 1] - g) / (2.0 * sym.e[l]);
    let mut r = libm::hypot(p, 1.0);
    if p < 0.0 {
        r = -r;
    }
    sym.d[l] = sym.e[l] / (p + r);
    sym.d[l + 1] = sym.e[l] * (p + r);
    let dl1 = sym.d[l + 1];
    let mut h = g - sym.d[l];
    for i in (l + 2)..n {
        sym.d[i] -= h;
    }
    *f += h;

    p = sym.d[m];
    let (mut c, mut c2, mut c3) = (1.0, 1.0, 1.0);
    let el1 = sym.e[l + 1];
    let (mut s, mut s2) = (0.0, 0.0);
    for i in (l..m).rev() {
        c3 = c2;
        c2 = c;
        s2 = s;
        let g = c * sym.e[i];
        h = c * p;
        r = libm::hypot(p, sym.e[i]);
        sym.e[i + 1] = s * r;
        s = sym.e[i] / r;
        c = p / r;
        p = c * sym.d[i] - s * g;
        sym.d[i + 1] = h + s * (c * g + s * sym.d[i]);
        tql2_rotate_columns(sym, i, s, c);
    }
    p = -s * s2 * c3 * el1 * sym.e[l] / dl1;
    sym.e[l] = s * p;
    sym.d[l] = c * p;
}

/// Applies the plane rotation `(s, c)` to columns `i` and `i + 1` of `V`, every row.
fn tql2_rotate_columns(sym: &mut Sym, i: usize, s: f64, c: f64) {
    for k in 0..sym.n {
        let h = sym.get(k, i + 1);
        sym.set(k, i + 1, s * sym.get(k, i) + c * h);
        sym.set(k, i, c * sym.get(k, i) - s * h);
    }
}

/// Replaces JAMA's selection sort with a **stable** sort on `(value.total_cmp, index)`
/// (D5), permuting `V`'s columns identically.
fn tql2_sort(sym: &mut Sym) {
    let n = sym.n;
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&a, &b| sym.d[a].total_cmp(&sym.d[b]).then(a.cmp(&b)));
    let old_d = sym.d.clone();
    let old_v = sym.v.clone();
    for (new_col, &old_col) in order.iter().enumerate() {
        sym.d[new_col] = old_d[old_col];
        for row in 0..n {
            sym.set(row, new_col, old_v[row * n + old_col]);
        }
    }
}
