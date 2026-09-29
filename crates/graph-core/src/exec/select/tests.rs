//! The tier table's own tests: what `select` chooses from thresholds alone, and what it
//! refuses. Split from `select.rs` for the house 300-line cap — the same split
//! `partition.rs`/`partition/tests.rs` uses, and the tests are about the function's
//! *decisions* rather than its shape, so they read better together than interleaved.

use super::*;

/// A host with every tier but the GPU.
const FULL: Caps = Caps {
    simd: true,
    workers: 4,
    webgpu: false,
};

#[test]
fn auto_stays_scalar_below_every_threshold_and_promotes_at_the_threshold_itself() {
    let t = Thresholds {
        simd_nodes: 1_000,
        threads_nodes: 10_000,
        threads_max: 4,
    };
    assert_eq!(select(0, 0, FULL, t), Tier::Scalar);
    assert_eq!(select(999, 0, FULL, t), Tier::Scalar);
    assert_eq!(
        select(1_000, 0, FULL, t),
        Tier::Simd,
        "the threshold is `>=`"
    );
    assert_eq!(select(9_999, 0, FULL, t), Tier::Simd);
    assert_eq!(select(10_000, 0, FULL, t), Tier::Threads(4));
}

#[test]
fn auto_caps_the_worker_count_at_the_tables_maximum() {
    let t = Thresholds {
        simd_nodes: 1_000,
        threads_nodes: 100,
        threads_max: 2,
    };
    let seven = Caps { workers: 7, ..FULL };
    assert_eq!(select(10_000, 0, seven, t), Tier::Threads(2));
}

#[test]
fn auto_never_selects_the_gpu_however_large_the_graph_or_wide_the_host() {
    let t = Thresholds {
        simd_nodes: 1,
        threads_nodes: 1,
        threads_max: 64,
    };
    let gpu = Caps {
        webgpu: true,
        ..FULL
    };
    // Every threshold crossed at the smallest graph that exists, and the GPU in hand.
    assert_eq!(select(1, 0, gpu, t), Tier::Threads(4));
    // And at the largest graph the `u32` index space can hold.
    assert_ne!(select(u32::MAX as u64, 0, gpu, t), Tier::Gpu);
}

#[test]
fn a_tier_the_host_cannot_run_is_not_selected_even_past_its_threshold() {
    let t = Thresholds {
        simd_nodes: 10,
        threads_nodes: 10,
        threads_max: 4,
    };
    let scalar_only = Caps::NONE;
    assert_eq!(select(10_000, 0, scalar_only, t), Tier::Scalar);
    let one_worker = Caps {
        simd: false,
        workers: 1,
        webgpu: false,
    };
    assert_eq!(select(10_000, 0, one_worker, t), Tier::Scalar);
    let no_simd = Caps {
        simd: false,
        workers: 4,
        webgpu: false,
    };
    assert_eq!(select(10_000, 0, no_simd, t), Tier::Threads(4));
}

#[test]
fn the_committed_table_promotes_nothing_until_it_is_measured() {
    // Every node count the `u32` index space can hold, and nothing is promoted. `n` is a
    // node count, so `u64::MAX` is outside the domain and is not asserted here.
    for n in [0_u64, 220, 10_000, u32::MAX as u64] {
        assert_eq!(
            select(n, n, FULL, Thresholds::MEASURED),
            Tier::Scalar,
            "at n={n}"
        );
    }
}

#[test]
fn a_named_available_tier_is_honoured_at_any_size() {
    // The escape hatch: below every threshold, naming a tier is still how you get it.
    assert_eq!(
        resolve(Exec::Named(Tier::Simd), 3, 0, FULL, Thresholds::MEASURED),
        Ok(Tier::Simd)
    );
    assert_eq!(
        resolve(
            Exec::Named(Tier::Threads(4)),
            3,
            0,
            FULL,
            Thresholds::MEASURED
        ),
        Ok(Tier::Threads(4))
    );
}

#[test]
fn a_named_unavailable_tier_is_refused_rather_than_downgraded() {
    for tier in [Tier::Simd, Tier::Threads(4), Tier::Gpu] {
        let err = resolve(
            Exec::Named(tier),
            10_000,
            0,
            Caps::NONE,
            Thresholds::MEASURED,
        );
        assert_eq!(
            err,
            Err(SelectError::Unavailable {
                tier,
                caps: Caps::NONE
            })
        );
        assert!(err.unwrap_err().to_string().contains("not available"));
    }
}

#[test]
fn one_worker_is_not_threads_and_scalar_needs_nothing() {
    let one = Caps {
        simd: true,
        workers: 1,
        webgpu: false,
    };
    assert!(!one.can_run(Tier::Threads(2)));
    assert!(one.can_run(Tier::Simd));
    assert!(Caps::NONE.can_run(Tier::Scalar));
    assert_eq!(
        resolve(
            Exec::Named(Tier::Threads(1)),
            0,
            0,
            one,
            Thresholds::MEASURED
        ),
        Err(SelectError::Unavailable {
            tier: Tier::Threads(1),
            caps: one
        })
    );
}

#[test]
fn the_gpu_is_reachable_only_by_name_and_only_where_one_exists() {
    let gpu = Caps {
        webgpu: true,
        ..Caps::NONE
    };
    assert_eq!(
        resolve(Exec::Named(Tier::Gpu), 0, 0, gpu, Thresholds::MEASURED),
        Ok(Tier::Gpu)
    );
    assert!(
        resolve(
            Exec::Named(Tier::Gpu),
            0,
            0,
            Caps::NONE,
            Thresholds::MEASURED
        )
        .is_err()
    );
}

#[test]
fn auto_agrees_with_resolve_auto() {
    let t = Thresholds::MEASURED;
    for n in [0_u64, 1, 220, 10_000] {
        assert_eq!(
            resolve(Exec::Auto, n, 5, FULL, t),
            Ok(select(n, 5, FULL, t))
        );
    }
}
