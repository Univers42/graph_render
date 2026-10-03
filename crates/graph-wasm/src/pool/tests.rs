//! The pool against [`Serial`], natively, with `std::thread` helpers in place of wasm
//! instances: the same `Mutex`/`Condvar` code the shared-memory build runs.

use super::{Pool, PoolRunner, with_pool};
use graph_core::exec::{Runner, Serial, StepRange};
use graph_core::layout::force::{BarnesHut, ForceParams, ParticleMesh};
use graph_core::{REFERENCE_DEGREE, Topology, index_model, seeded_model};
use std::ops::Range;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc;
use std::thread::ThreadId;
use std::time::{Duration, Instant};

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

/// Writes `i * 3 + 1` at `i` and counts the calls that wrote each element.
struct Count(Vec<AtomicU32>);

impl Count {
    fn new(len: u32) -> Self {
        Self((0..len).map(|_| AtomicU32::new(0)).collect())
    }
}

impl StepRange for Count {
    type Out = u32;

    fn len(&self) -> u32 {
        u32::try_from(self.0.len()).expect("a test column fits in u32")
    }

    fn step_range(&self, range: Range<u32>, out: &mut [u32]) {
        for (i, slot) in range.zip(out) {
            self.0[i as usize].fetch_add(1, Ordering::Relaxed);
            *slot = i * 3 + 1;
        }
    }
}

#[test]
fn every_element_is_written_once_at_an_uneven_length() {
    with_pool(6, |pool| {
        let runner = PoolRunner {
            pool,
            skip_last: false,
        };
        for workers in [2, 3, 7] {
            let kernel = Count::new(1009);
            let mut out = Vec::new();
            runner.run(&kernel, workers, &mut out);
            assert!(out.iter().zip(0..).all(|(&v, i)| v == i * 3 + 1));
            let counts = kernel.0.iter().map(|c| c.load(Ordering::Relaxed));
            assert!(counts.into_iter().all(|c| c == 1), "{workers} workers");
        }
    });
}

// A helper the OS never schedules: registered, never serving. Before perf-p3-steal the pass
// waited for every addressed helper to wake, so this hung forever. The pool is leaked
// because a hung pass would outlive any borrow of it.
#[test]
fn a_helper_that_never_wakes_does_not_hold_the_pass() {
    let pool: &'static Pool = Box::leak(Box::new(Pool::new()));
    pool.lock().helpers += 1;
    let helper = std::thread::spawn(|| pool.serve());
    while pool.helpers() < 2 {
        std::thread::yield_now();
    }
    let (sent, received) = mpsc::channel();
    std::thread::spawn(move || {
        let runner = PoolRunner {
            pool,
            skip_last: false,
        };
        let mut out = Vec::new();
        runner.run(&Count::new(64), 3, &mut out);
        // The receiver is gone only once the test has already failed on its timeout.
        sent.send(out).ok();
    });
    let out = received.recv_timeout(Duration::from_secs(20));
    pool.close();
    helper.join().expect("the helper leaves on close");
    let out = out.expect("the pass waited for a helper that never woke");
    assert!(out.iter().zip(0..).all(|(&v, i)| v == i * 3 + 1));
}

/// Writes `i + 1` at `i`. The coordinator does not leave its chunk before a helper has
/// entered the pass, and every helper call sleeps before it writes, so the coordinator runs
/// out of chunks while a helper is still inside: only the join keeps that span from reading
/// back as 0. With `panic_on_helper`, a helper call panics instead of writing.
struct Slow {
    coordinator: ThreadId,
    helper_in: AtomicBool,
    panic_on_helper: bool,
}

impl Slow {
    fn new(panic_on_helper: bool) -> Self {
        Self {
            coordinator: std::thread::current().id(),
            helper_in: AtomicBool::new(false),
            panic_on_helper,
        }
    }
}

impl StepRange for Slow {
    type Out = u32;

    fn len(&self) -> u32 {
        64
    }

    fn step_range(&self, range: Range<u32>, out: &mut [u32]) {
        if std::thread::current().id() == self.coordinator {
            let deadline = Instant::now() + Duration::from_secs(10);
            while !self.helper_in.load(Ordering::Acquire) {
                assert!(Instant::now() < deadline, "no helper entered the pass");
                std::thread::yield_now();
            }
        } else {
            self.helper_in.store(true, Ordering::Release);
            assert!(!self.panic_on_helper, "a helper fails mid-pass");
            std::thread::sleep(Duration::from_millis(50));
        }
        for (i, slot) in range.zip(out) {
            *slot = i + 1;
        }
    }
}

#[test]
fn the_pass_waits_for_a_helper_still_inside_it() {
    with_pool(3, |pool| {
        let runner = PoolRunner {
            pool,
            skip_last: false,
        };
        let mut out = Vec::new();
        runner.run(&Slow::new(false), 4, &mut out);
        assert!(out.iter().zip(1..).all(|(&v, i)| v == i));
    });
}

#[test]
#[should_panic(expected = "a helper panicked inside a pass")]
fn a_helper_panic_fails_the_pass_instead_of_hanging_it() {
    with_pool(3, |pool| {
        let runner = PoolRunner {
            pool,
            skip_last: false,
        };
        runner.run(&Slow::new(true), 4, &mut Vec::new());
    });
}
