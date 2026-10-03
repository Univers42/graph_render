use super::*;
use crate::post::mingle::level::endpoints;

#[test]
fn a_round_whose_first_pass_merges_nothing_stops_after_that_pass() {
    // R16. Two short edges a hundred apart: no merge gains ink, so the first pass takes
    // nothing. The level is then unchanged, and so is every score of the next pass. The
    // reference stops there (`mingle.py:370-371`: `if pairs.shape[0] == 0: break`).
    let pos: Vec<P> = vec![[0.0, 0.0], [1.0, 0.0], [100.0, 100.0], [100.0, 101.0]];
    let ends = endpoints(&pos, &[0, 2], &[1, 3]).expect("dense");
    let mut run = Run::new(&ends);
    let eps = SOLVE_EPS_FRAC * diagonal(&pos);
    assert!(
        !run.round(&Params::default().clamped(), eps),
        "nothing merged"
    );
    assert_eq!(run.merges(), 0);
    assert_eq!(run.scored_passes, 1, "one pass, as the reference runs");
}

#[test]
fn the_declared_proximity_term_carries_the_row_sort_the_oracle_also_does() {
    // R15. The oracle the registry names is `SciGraphs/.../bundling/mingle.py`, whose
    // `knn_numpy` sorts each bundle's whole row before keeping k (`mingle.py:109-112`):
    // `out[s:e, :kk] = np.argsort(d, axis=1, kind="stable")[:, :kk]`. `nearest` does the
    // same with `sort_by` then `truncate`: m rows of an (m − 1)-long sort, m² log m.
    assert!(
        META.complexity.contains("m^2 log m proximity"),
        "{}",
        META.complexity
    );
    assert!(
        META.degradation.contains("O(m^2 log m)"),
        "{}",
        META.degradation
    );
}
