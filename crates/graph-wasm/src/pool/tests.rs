//! The pool against [`Serial`], natively, with `std::thread` helpers in place of wasm
//! instances: the same `Mutex`/`Condvar` code the shared-memory build runs.

use super::{PoolRunner, with_pool};
use graph_core::exec::{Runner, Serial};
use graph_core::layout::force::{BarnesHut, ForceParams, ParticleMesh};
use graph_core::{REFERENCE_DEGREE, Topology, index_model, seeded_model};

fn model(n: u32) -> Topology {
    let (nodes, edges) = seeded_model(7, n, REFERENCE_DEGREE);
    index_model(&nodes, &edges).expect("the seeded model indexes")
}

fn bytes(topology: &Topology, layout: &str, runner: &impl Runner, workers: u32) -> Vec<u8> {
    let params = ForceParams::default();
    let geometry = match layout {
        "bh" => BarnesHut::run_with(topology, &params, runner, workers),
        _ => ParticleMesh::run_with(topology, &params, runner, workers),
    };
    let geometry = geometry.expect("the layout runs");
    graph_core::layout::snapshot(topology, geometry)
        .expect("the snapshot builds")
        .to_bytes()
}

#[test]
fn every_worker_count_gives_the_serial_bytes() {
    let topology = model(301);
    for layout in ["bh", "pm"] {
        let serial = bytes(&topology, layout, &Serial, 1);
        with_pool(6, |pool| {
            let runner = PoolRunner {
                pool,
                skip_last: false,
            };
            for workers in [1, 2, 3, 4, 7] {
                let ran = bytes(&topology, layout, &runner, workers);
                assert!(
                    ran == serial,
                    "{layout} at {workers} workers differs from serial"
                );
            }
        });
    }
}

#[test]
fn workers_past_the_helpers_are_capped_and_still_equal() {
    let topology = model(150);
    let serial = bytes(&topology, "bh", &Serial, 1);
    with_pool(2, |pool| {
        let runner = PoolRunner {
            pool,
            skip_last: false,
        };
        assert!(bytes(&topology, "bh", &runner, 7) == serial);
    });
}

#[test]
fn the_negative_control_changes_the_bytes() {
    let topology = model(150);
    let serial = bytes(&topology, "bh", &Serial, 1);
    with_pool(3, |pool| {
        let runner = PoolRunner {
            pool,
            skip_last: true,
        };
        assert!(bytes(&topology, "bh", &runner, 4) != serial);
    });
}

#[test]
fn no_helper_runs_serially() {
    let topology = model(40);
    with_pool(0, |pool| {
        let runner = PoolRunner {
            pool,
            skip_last: false,
        };
        assert!(bytes(&topology, "bh", &runner, 4) == bytes(&topology, "bh", &Serial, 1));
    });
}
