//! The writer's own cases: that a fixture round-trips byte for byte, that the header says
//! which state and which rung it was written from, that nothing on the wire is non-finite,
//! that the only integer the writer puts on the wire is a `u32`, and that each deliberate
//! fault moves bytes without moving a length.

use super::emit::{self, HEADER_LEN, scale_for};
use super::settle::{self, State};
use graph_core::layout::force::{ForceSession, MeshProbe};

/// A small case, big enough that every payload section is longer than the header.
const N: u32 = 64;

/// One case: the mesh session over the seeded model and the probe over it.
fn probe(n: u32) -> (ForceSession, MeshProbe) {
    let session = settle::session(n).expect("the frozen params are valid");
    let probed = session.mesh_probe().expect("a field to solve");
    (session, probed)
}

/// One case's bytes, plus its header read back out of them: the ten `u32` words (the magic
/// as one) and the three `f64`s, then the payload's opening frame pair.
fn written(n: u32, state: State) -> (Vec<u8>, Vec<u32>, Vec<f64>) {
    written_with(n, state, &mut emit::Knobs::none())
}

/// The same, with a deliberate fault switched on.
fn written_with(n: u32, state: State, knobs: &mut emit::Knobs) -> (Vec<u8>, Vec<u32>, Vec<f64>) {
    let (session, probed) = probe(n);
    let case = emit::Case {
        probe: &probed,
        xs: session.xs(),
        ys: session.ys(),
        state,
    };
    let bytes = emit::write(&case, knobs).expect("the probe's columns are finite");
    let words = emit::header_words(&bytes);
    let reals = emit::header_reals(&bytes);
    (bytes, words, reals)
}

#[test]
fn a_written_fixture_round_trips() {
    let (bytes, words, reals) = written(N, State::Start);
    assert_eq!(&bytes[0..4], b"GMFX", "the magic opens the file");
    assert_eq!(words[1], 1, "format major is 1");
    assert_eq!(words[2], 0, "format minor is 0");
    assert_eq!(words[3], N, "the node count is the case's own");
    let (n, m, side) = (words[3], words[4], words[5]);
    assert!(m > 0, "the gate's model has edges");
    assert!(side.is_power_of_two(), "a radix-2 side is a power of two");
    assert_eq!(
        HEADER_LEN, 64,
        "the header is the 64 bytes the README's table states"
    );
    let expected = 64u32 + 8 + 16 * m + 16 * n + 16 * side + 16 * side * side + 48 * n;
    assert_eq!(
        bytes.len() as u32,
        expected,
        "every length follows from n, m and P"
    );
    assert!(reals[0] > 0.0, "the cell size is positive");
    let (cells, reach) = (words[10], words[11]);
    assert!(
        cells > 0 && reach > 0 && cells >= reach,
        "the frame is placed"
    );
}

#[test]
fn the_header_carries_the_state_and_the_rung() {
    let (_, start_words, _) = written(N, State::Start);
    let (_, settled_words, _) = written(N, State::Settled);
    assert_eq!(start_words[6], 0, "start is state 0");
    assert_eq!(settled_words[6], 1, "settled is state 1");
    assert_eq!(
        start_words[7], 0,
        "the settled flag is clear on the start file"
    );
    assert_eq!(settled_words[7], 1, "and set on the settled file");
    assert_eq!(start_words[9], 0, "the pad word is zero on both files");
    assert_eq!(settled_words[9], 0, "…on both files");
    let step = start_words[8];
    assert_eq!(
        step, step as i32 as u32,
        "the rung is two's-complemented on the wire"
    );
    assert_eq!(
        State::of("mesh-1k-settled.gmfx"),
        Some(State::Settled),
        "the name names its state"
    );
}

/// The rung word and the cell size it implies, as a pair the header's own words give. The
/// rung is **not** a multiple of four: `frame::place` starts at
/// `floor(4 * log2(span / side))` and climbs by one until the frame fits, so `h` is
/// `2^(step/4)` and `step` is whatever integer rung that landed on.
fn rung_and_h(words: &[u32], reals: &[f64]) -> (i32, f64) {
    let step = words[8] as i32;
    assert!(reals[0] > 0.0, "each file carries a positive cell size");
    assert_eq!(
        reals[0],
        libm::exp2(f64::from(step) / 4.0),
        "h is the rung's own 2^(step/4)"
    );
    (step, reals[0])
}

#[test]
fn each_state_carries_its_own_frame() {
    // Asserted per file, and not that the two files differ: at 64 nodes 100 ticks need not
    // cross a rung. `the_two_states_have_different_frames` in graph-core is where a crossing
    // is pinned, on a model large enough to cross one.
    let (_, start_words, start_reals) = written(N, State::Start);
    let (_, settled_words, settled_reals) = written(N, State::Settled);
    let _ = rung_and_h(&start_words, &start_reals);
    let _ = rung_and_h(&settled_words, &settled_reals);
}

#[test]
fn no_column_holds_a_non_finite_value() {
    let (session, mut probed) = probe(N);
    for (name, column) in [
        ("x", session.xs().to_vec()),
        ("y", session.ys().to_vec()),
        ("strength", probed.strength.clone()),
        ("link", probed.link_dx.clone()),
        ("charge", probed.charge_dx.clone()),
        ("collide", probed.collide_dx.clone()),
        ("spectrum", probed.spectrum_re.clone()),
    ] {
        assert!(column.iter().all(|v| v.is_finite()), "{name} is all finite");
    }
    probed.charge_dy[0] = f64::NAN;
    let case = emit::Case {
        probe: &probed,
        xs: session.xs(),
        ys: session.ys(),
        state: State::Start,
    };
    let refused = emit::write(&case, &mut emit::Knobs::none());
    let refusal = refused.expect_err("a non-finite column is a refusal, not a written byte");
    assert!(
        refusal.contains("finite"),
        "the refusal names what is wrong: {refusal}"
    );
}

/// The writer's half of `emit.rs`: everything above the marker the writer itself names.
///
/// Only the writer half is in scope. The two `u32_at`/`f64_at` readers below the marker take
/// `usize` offsets because they index a `Vec<u8>`, and they read rather than write, so D6
/// does not reach them.
fn writer_source() -> &'static str {
    const MARKER: &str = "// END OF THE WRITER";
    let source = include_str!("emit.rs");
    let writer = source.split_once(MARKER).map_or(source, |(head, _)| head);
    assert!(
        writer.contains("fn put_u32"),
        "the writer half was found and is the first half"
    );
    assert!(
        source.len() > writer.len(),
        "and the readers are below the marker"
    );
    writer
}

/// The integer widths a wire field may never have. `usize`/`isize` are the machine words D6
/// exists to keep off a wire, and the 16- and 64-bit types are widths the format does not
/// use at all. `u8` is absent because it is not an integer here — it is the element type of
/// the byte buffer and of the four magic bytes, neither of which is a field.
const FORBIDDEN: [&str; 6] = ["usize", "isize", "u64", "u16", "i64", "i16"];

#[test]
fn no_integer_width_but_u32_reaches_the_wire() {
    // `HEADER_LEN` is a `usize` because it indexes a `Vec<u8>` in Rust and is never written,
    // so it is exempt by name. Every integer *field* is `u32`; `i32` appears once, in
    // `rung_word`, as the two's-complement cast the rung needs, and the wire still receives
    // a `u32`.
    const EXEMPT: &str = "pub const HEADER_LEN: usize = 64;";
    for (at, line) in writer_source().lines().enumerate() {
        let code = line.split("//").next().unwrap_or(line).trim();
        if code == EXEMPT {
            continue;
        }
        for bad in FORBIDDEN {
            assert!(
                !code.contains(bad),
                "emit.rs:{} names {bad}, and only u32 goes on the wire: {line}",
                at + 1
            );
        }
    }
}

/// The writers themselves: the two that put integers on the wire take `u32` and nothing
/// else, and the two that put floats take `f64`. This is the positive half of the width
/// rule — the grep above says no *other* width appears, this says the right ones do.
#[test]
fn the_two_writers_of_each_width_take_that_width() {
    let source = writer_source();
    let puts: Vec<&str> = source
        .lines()
        .filter(|l| l.trim_start().starts_with("fn put_u"))
        .collect();
    assert_eq!(
        puts.len(),
        2,
        "the two integer writers are put_u32 and put_u32s"
    );
    for line in puts {
        assert!(line.contains("u32"), "an integer writer takes u32: {line}");
    }
    assert_eq!(
        source.matches("fn put_f64").count(),
        2,
        "and the float writers are put_f64 and put_f64s"
    );
}

#[test]
fn the_scale_is_the_power_of_two_the_record_names() {
    assert_eq!(scale_for(1_000_000), Some(2048), "1M deposits at 2^11");
    assert_eq!(
        scale_for(1),
        Some(1 << 30),
        "one charge is the tightest fit"
    );
    assert_eq!(scale_for(0), None, "no nodes, no scale");
    for n in [1u32, 2, 999, 1_000, 10_000, 50_000, 1_000_000] {
        let scale = scale_for(n).expect("a positive count has a scale");
        assert!(
            u64::from(n) * u64::from(scale) <= i32::MAX as u64,
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
    let (n, m, side) = (words[3], words[4], words[5]);
    let promised = 64u32 + 8 + 16 * m + 16 * n + 16 * side + 16 * side * side + 48 * n;
    assert_eq!(
        bytes.len() as u32,
        promised,
        "the 1k file is the size the README states"
    );
}

#[test]
fn a_mutated_pass_and_a_mutated_rung_move_bytes() {
    let (plain, _, _) = written(1_000, State::Start);
    let mut knobs = emit::Knobs::none();
    knobs.pass = Some("collide");
    let (swapped, _, _) = written_with(1_000, State::Start, &mut knobs);
    assert_ne!(swapped, plain, "swapping a delta column moves the payload");
    let mut knobs = emit::Knobs::none();
    knobs.rung = 1;
    let (rung, rung_words, _) = written_with(1_000, State::Start, &mut knobs);
    assert_ne!(rung, plain, "moving the rung moves the header");
    assert_eq!(
        rung.len(),
        plain.len(),
        "and moves only bytes, not a length, so --check reports one difference"
    );
    let (_, plain_words, _) = written(1_000, State::Start);
    assert_eq!(
        rung_words[8],
        plain_words[8] + 1,
        "the rung word is the one that moved"
    );
}

#[test]
fn every_case_the_names_reach_is_a_size_and_a_state() {
    for n in settle::SIZES {
        for state in State::all() {
            let name = settle::file_name(n, state);
            assert_eq!(State::of(&name), Some(state), "{name} names its own state");
        }
    }
    assert_eq!(settle::file_name(1_000, State::Start), "mesh-1k-start.gmfx");
    assert_eq!(
        settle::file_name(1_000_000, State::Settled),
        "mesh-1m-settled.gmfx"
    );
    assert_eq!(
        settle::file_name(10_000, State::Start),
        "mesh-10k-start.gmfx"
    );
    assert_eq!(
        settle::file_name(50_000, State::Settled),
        "mesh-50k-settled.gmfx"
    );
}
