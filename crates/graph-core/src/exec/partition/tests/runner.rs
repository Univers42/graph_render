//! [`Serial`]: the reference every other executor is measured against.
//!
//! It is a real tier rather than a stand-in — `run` with `workers = 1` is the tier 1a path
//! — so these tests are about the *runner*, not about a mock: whatever worker count the
//! host reports, the bytes are the whole-run bytes, and the caller's buffer is overwritten
//! rather than appended to.

use super::super::{Runner, Serial, StepRange};
use super::contract::Column;

/// The reference runner's claim: whatever worker count the host reports, the bytes are
/// the whole-run bytes. `Serial` is the tier 1a path *and* the yardstick for every other
/// runner, so this is the test a threaded executor is measured against.
#[test]
fn the_serial_runner_gives_the_whole_run_bytes_at_every_worker_count() {
    let column = Column::fixture();
    let n = column.len() as usize;
    let mut reference = Vec::new();
    Serial.run(&column, 1, &mut reference);
    assert_eq!(reference.len(), n);
    for workers in [0_u32, 1, 2, 3, 4, 7, 16] {
        let mut out = vec![7.0; n];
        Serial.run(&column, workers, &mut out);
        assert_eq!(out, reference, "workers={workers}");
    }
}

/// `run` clears and overwrites: a caller's buffer is never appended to, and never keeps a
/// value the kernel did not write. Both halves matter — an executor that forgot to clear
/// would double every column.
#[test]
fn the_runner_clears_the_callers_buffer_rather_than_appending_to_it() {
    let column = Column {
        values: vec![1.0, 2.0, 3.0],
    };
    let mut out = vec![99.0; 3];
    Serial.run(&column, 2, &mut out);
    assert_eq!(out.len(), 3);
    assert_ne!(out, vec![99.0; 3]);
    // A kernel of no outputs empties the buffer, whatever was in it.
    let none = Column { values: Vec::new() };
    Serial.run(&none, 4, &mut out);
    assert!(out.is_empty());
}
