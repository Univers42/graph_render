//! `tred2` — Householder reduction to tridiagonal form, split out of `dense_sym.rs` to
//! stay under the house line cap. Ported line-for-line from JAMA `tred2`
//! (`docs/decisions/eigensolver.md`); `Sym`'s fields are its parent module's, visible
//! here because this is a descendant module, not a different crate.

use super::Sym;

/// Symmetric Householder reduction to tridiagonal form (JAMA `tred2`).
pub(super) fn tred2(sym: &mut Sym) {
    let n = sym.n;
    for j in 0..n {
        sym.d[j] = sym.get(n - 1, j);
    }
    for i in (1..n).rev() {
        let h = tred2_row(sym, i);
        sym.d[i] = h;
    }
    tred2_accumulate(sym);
}

/// One `i` step of `tred2`: scales, and either the trivial `scale == 0` copy or the full
/// Householder generation and similarity transformation. Returns the new `d[i]` (`h`).
fn tred2_row(sym: &mut Sym, i: usize) -> f64 {
    let scale: f64 = (0..i).map(|k| sym.d[k].abs()).sum();
    if scale == 0.0 {
        sym.e[i] = sym.d[i - 1];
        for j in 0..i {
            sym.d[j] = sym.get(i - 1, j);
            sym.set(i, j, 0.0);
            sym.set(j, i, 0.0);
        }
        return 0.0;
    }
    tred2_householder(sym, i, scale)
}

/// Generates the Householder vector for row/column `i` and applies the similarity
/// transformation to the remaining `0..i` block. JAMA `tred2`, the `else` branch.
fn tred2_householder(sym: &mut Sym, i: usize, scale: f64) -> f64 {
    let mut h = 0.0;
    for k in 0..i {
        sym.d[k] /= scale;
        h += sym.d[k] * sym.d[k];
    }
    let mut f = sym.d[i - 1];
    let mut g = h.sqrt();
    if f > 0.0 {
        g = -g;
    }
    sym.e[i] = scale * g;
    h -= f * g;
    sym.d[i - 1] = f - g;
    for j in 0..i {
        sym.e[j] = 0.0;
    }
    tred2_transform_columns(sym, i);
    f = 0.0;
    for j in 0..i {
        sym.e[j] /= h;
        f += sym.e[j] * sym.d[j];
    }
    let hh = f / (h + h);
    for j in 0..i {
        sym.e[j] -= hh * sym.d[j];
    }
    tred2_update_block(sym, i);
    h
}

/// The middle third of `tred2`'s `else` branch: `e[j] += V[k][j]*f` accumulation.
fn tred2_transform_columns(sym: &mut Sym, i: usize) {
    for j in 0..i {
        let f = sym.d[j];
        sym.set(j, i, f);
        let mut g = sym.e[j] + sym.get(j, j) * f;
        for k in (j + 1)..i {
            g += sym.get(k, j) * sym.d[k];
            sym.e[k] += sym.get(k, j) * f;
        }
        sym.e[j] = g;
    }
}

/// The last third of `tred2`'s `else` branch: the rank-2 update of the `0..i` block.
fn tred2_update_block(sym: &mut Sym, i: usize) {
    for j in 0..i {
        let f = sym.d[j];
        let g = sym.e[j];
        for k in j..i {
            let updated = sym.get(k, j) - (f * sym.e[k] + g * sym.d[k]);
            sym.set(k, j, updated);
        }
        sym.d[j] = sym.get(i - 1, j);
        sym.set(i, j, 0.0);
    }
}

/// Accumulates the Householder reflections into `V` (JAMA `tred2`'s trailing loop).
fn tred2_accumulate(sym: &mut Sym) {
    let n = sym.n;
    if n < 2 {
        if n == 1 {
            sym.set(0, 0, 1.0);
        }
        return;
    }
    for i in 0..(n - 1) {
        tred2_accumulate_step(sym, i);
    }
    for j in 0..n {
        sym.d[j] = sym.get(n - 1, j);
        sym.set(n - 1, j, 0.0);
    }
    sym.set(n - 1, n - 1, 1.0);
    sym.e[0] = 0.0;
}

fn tred2_accumulate_step(sym: &mut Sym, i: usize) {
    let n = sym.n;
    sym.set(n - 1, i, sym.get(i, i));
    sym.set(i, i, 1.0);
    let h = sym.d[i + 1];
    if h != 0.0 {
        for k in 0..=i {
            sym.d[k] = sym.get(k, i + 1) / h;
        }
        for j in 0..=i {
            let g: f64 = (0..=i).map(|k| sym.get(k, i + 1) * sym.get(k, j)).sum();
            for k in 0..=i {
                let updated = sym.get(k, j) - g * sym.d[k];
                sym.set(k, j, updated);
            }
        }
    }
    for k in 0..=i {
        sym.set(k, i + 1, 0.0);
    }
}
