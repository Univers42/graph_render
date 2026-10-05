//! The writer's own cases: that a fixture round-trips byte for byte, that the header says
//! which state and which rung it was written from, that nothing on the wire is non-finite,
//! and that the only integer the writer puts on the wire is a `u32`.

use super::*;
use crate::gpu_fixtures::settle;

/// A 64-node case, small enough to build in a test and large enough that every section of
/// the payload is longer than the header.
fn probe(n: u32) -> (ForceSession, MeshProbe) {
    let session = settle::session(n).expect("the frozen params are valid");
    let probe = session.mesh_probe().expect("a field to solve");
    (session, probe)
}

/// The bytes one case writes, plus the header's own words read back out of them.
fn written(n: u32, state: State) -> (Vec<u8>, Vec<u64>, Vec<f64>) {
    let (session, probe) = probe(n);
    let bytes = emit::write(
        &probe,
        session.xs(),
        session.ys(),
        state,
        &mut emit::Knobs::none(),
    )
    .expect("the probe's columns are finite");
    (bytes, emit::header_words(&bytes), emit::header_reals(&bytes))
}

/// The bytes one case writes with a deliberate fault switched on.
fn written_with(knobs: &mut emit::Knobs, state: State) -> Vec<u8> {
    let (session, probe) = probe(1_000);
    emit::write(&probe, session.xs(), session.ys(), state, knobs).expect("finite")
}

#[test]
fn a_written_fixture_round_trips() {
    let (bytes, words, reals) = written(64, State::Start);
    assert_eq!(&bytes[0..4], b"GMFX", "the magic opens the file");
    assert_eq!(words[1], 1, "format major is 1");
    assert_eq!(words[2], 0, "format minor is 0");
    assert_eq!(words[3], 64, "the node count is the case's own");
    assert_eq!(words[4] as usize, words.len(), "the edge count is a column length");
    let side = words[5];
    let cells = words[words.len() - 2];
    let reach = words[words.len() - 1];
    let expected = HEADER_LEN + 8 + 16 * words[4] + 64 * words[3] + 16 * side + 16 * side * side;
    assert_eq!(bytes.len(), expected as usize, "every length follows from n, m and P");
    assert!(cells > 0 && reach > 0 && cells >= reach, "the frame is placed");
    assert!(reals[3] > 0.0, "the cell size is positive");
}

#[test]
fn the_header_carries_the_state_and_the_rung() {
    let (_, start_words, _) = written(64, State::Start);
    let (_, settled_words, _) = written(64, State::Settled);
    assert_eq!(start_words[6], 0, "start is state 0");
    assert_eq!(settled_words[6], 1, "settled is state 1");
    assert_eq!(start_words[7], 0, "the flag bit is clear on the start file");
    assert_eq!(settled_words[7], 1, "and set on the settled file");
    assert_eq!(start_words[9], 0, "the pad word is zero");
    assert_eq!(settled_words[9], 0, "on both files");
    let step = start_words[8];
    assert_eq!(step, (start_words[8] as i32) as u32, "the rung is two's-complemented on the wire");
    let (_, _, settled_reals) = written(64, State::Settled);
    let (_, _, start_reals) = written(64, State::Start);
    assert_ne!(
        settled_reals[3].to_bits(),
        start_reals[3].to_bits(),
        "the settled state carries its own frame, not the start one's"
    );
}

#[test]
fn no_column_holds_a_non_finite_value() {
    let (session, mut probe) = probe(64);
    for (name, column) in [
        ("x", session.xs().to_vec()),
        ("y", session.ys().to_vec()),
        ("strength", probe.strength.clone()),
        ("link", probe.link_dx.clone()),
        ("charge", probe.charge_dx.clone()),
        ("collide", probe.collide_dx.clone()),
    ] {
        assert!(column.iter().all(|v| v.is_finite()), "{name} is all finite");
    }
    probe.charge_dy[0] = f64::NAN;
    let refused = emit::write(&probe, session.xs(), session.ys(), State::Start, &mut emit::Knobs::none());
    assert!(refused.is_err(), "a non-finite column is a refusal, not a written byte");
    assert!(
        refused.unwrap_err().contains("finite"),
        "the refusal names what is wrong: {}",
        refused.unwrap_err()
    );
}

#[test]
fn every_integer_on_the_wire_is_u32() {
    let source = include_str!("emit.rs");
    for (at, line) in source.lines().enumerate() {
        let code = line.split("//").next().unwrap_or(line);
        for bad in ["usize", "u64", "i32", "u8", "u16", "i64"] {
            assert!(
                !code.contains(bad),
                "emit.rs:{} names {bad}, and only u32 goes on the wire: {line}",
                at + 1
            );
        }
    }
}

#[test]
fn the_scale_is_the_power_of_two_the_record_names() {
    assert_eq!(scale_for(1_000_000), Some(2048), "1M deposits at 2^11");
    assert_eq!(scale_for(1), Some(1 << 30), "one charge is the tightest fit");
    assert_eq!(scale_for(0), None, "no nodes, no scale");
    for n in [1u32, 2, 999, 1_000, 10_000, 50_000, 1_000_000] {
        let scale = scale_for(n).expect("a positive count has a scale");
        assert!(
            n as u64 * scale as u64 <= i32::MAX as u64,
            "n {n} at scale {scale} must fit an i32 cell"
        );
    }
}

#[test]
fn the_size_is_what_the_format_promises() {
    // The estimate in the README is `64 + 8 + 16m + 16n + 16P + 16P^2 + 48n`; this asserts
    // the writer agrees with that expression for the case it just built, so the README's
    // numbers and the bytes cannot drift apart silently.
    let (bytes, words, _) = written(1_000, State::Start);
    let (n, m, side) = (words[3] as u64, words[4] as u64, words[5] as u64);
    let promised = 64 + 8 + 16 * m + 16 * n + 16 * side + 16 * side * side + 48 * n;
    assert_eq!(bytes.len() as u64, promised, "the 1k file is the size the README states");
}

#[test]
fn a_mutated_pass_and_a_mutated_rung_move_bytes() {
    let (_, plain, _) = written(1_000, State::Start);
    let (session, probe) = probe(1_000);
    let mut knobs = emit::Knobs::none();
    knobs.pass = Some("collide");
    let swapped = emit::write(&probe, session.xs(), session.ys(), State::Start, &mut knobs)
        .expect("the mutation writes bytes too");
    assert_ne!(swapped, plain, "swapping a delta column moves the payload");
    let mut knobs = emit::Knobs::none();
    knobs.rung = 1;
    let rung = emit::write(&probe, session.xs(), session.ys(), State::Start, &mut knobs)
        .expect("the mutation writes bytes too");
    assert_ne!(rung, plain, "moving the rung moves the header");
}
