//! `tick_with` over the pool reaches the bits `tick` reaches, for both engines and every worker
//! count, through chunked calls; and the pool's negative control moves them. The wasm32 half is
//! `harness/wasm-threads.mjs session`, which drives the exports over a real shared memory.

use super::super::{Engine, create, reset, tick, tick_with};
use super::fixture::{bits, model, params, wire_of};
use crate::pool::{PoolRunner, with_pool};

const ENGINES: [Engine; 2] = [Engine::BarnesHut, Engine::ParticleMesh];

/// Both columns after five calls of ten ticks, through `drive` on a fresh session.
fn columns(engine: Engine, drive: impl Fn(u32)) -> (Vec<u64>, Vec<u64>) {
    reset();
    let id = create(&model(5, 301), params(), engine).expect("in range");
    for _ in 0..5 {
        drive(id);
    }
    (bits(&wire_of(id, 0)), bits(&wire_of(id, 1)))
}

#[test]
fn every_worker_count_ticks_the_serial_bits() {
    for engine in ENGINES {
        let serial = columns(engine, |id| {
            tick(id, 10).expect("runs");
        });
        with_pool(6, |pool| {
            let runner = PoolRunner {
                pool,
                skip_last: false,
            };
            for workers in [1, 2, 3, 4, 7] {
                let threaded = columns(engine, |id| {
                    tick_with(id, 10, &runner, workers).expect("runs");
                });
                assert!(threaded == serial, "{engine:?} at {workers} workers");
            }
        });
    }
}

/// Barnes-Hut only: in a debug build the mesh's own invariant check (`collide/gather.rs`)
/// refuses the broken pass with a panic before its bytes can differ. The mesh's negative control
/// is the release wasm32 one, `harness/wasm-threads.mjs session --break`.
#[test]
fn the_negative_control_moves_the_session() {
    let engine = Engine::BarnesHut;
    let serial = columns(engine, |id| {
        tick(id, 10).expect("runs");
    });
    with_pool(3, |pool| {
        let runner = PoolRunner {
            pool,
            skip_last: true,
        };
        let broken = columns(engine, |id| {
            tick_with(id, 10, &runner, 4).expect("runs");
        });
        assert!(broken != serial, "the last part wrote nothing");
    });
}
