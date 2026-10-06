//! One case to one `.gmfx`. Little-endian, every integer `u32` (D6), every value the CPU
//! mesh's own `f64` — the reference is never narrowed before an f32 arm is compared against
//! it, because at 12 000 units f32's spacing is 2^-10 (`gpu-force-tier.md:91-93`) and a
//! reference already narrowed could not show a 2^-11 difference.
//!
//! Caveat: the sizes in `fixtures/gpu/README.md` are estimates from `m ≈ 1.55n`; this file
//! prints every file's real byte count, and `gpu/fixture.ts` refuses a payload whose length
//! does not follow from the header, so a file far off its estimate is a load error rather
//! than a quiet misread.

use graph_core::layout::force::MeshProbe;

use super::settle::State;

/// The four magic bytes every `.gmfx` starts with.
pub const MAGIC: [u8; 4] = *b"GMFX";

/// Header length in bytes: ten `u32` words, then three `f64`s — no hole left open.
pub const HEADER_LEN: usize = 64;

/// The twelve `u32` words a written file opens with, read back out of it: the header's ten
/// (the magic as one) and then the payload's frame pair. Test-only: `--check` compares bytes
/// and never parses a header, so the writer's own tests are the only place a reader lives.
#[cfg(test)]
pub fn header_words(bytes: &[u8]) -> Vec<u32> {
    (0..12).map(|w| u32_at(bytes, w * 4)).collect()
}

/// The header's three `f64` words in wire order: `h`, `origin_x`, `origin_y`. Test-only, as
/// [`header_words`] is.
#[cfg(test)]
pub fn header_reals(bytes: &[u8]) -> Vec<f64> {
    (0..3).map(|w| f64_at(bytes, 40 + w * 8)).collect()
}

/// The deliberate wrongness `--check`'s negative controls switch on. Both are off by
/// default and both are read from the environment once per run, so an ordinary run cannot
/// reach them.
#[derive(Debug, Clone, Copy, Default)]
pub struct Knobs {
    /// Write the named pass's columns where the charge columns belong.
    pub pass: Option<&'static str>,
    /// Add this to the rung in the emitted header.
    pub rung: i32,
}

impl Knobs {
    /// The knobs an ordinary run has: none of them wrong. What the writer's own tests
    /// build, so a test never has to unset an inherited variable to be sure it is testing
    /// the honest writer.
    #[cfg(test)]
    pub fn none() -> Knobs {
        Knobs::default()
    }

    /// The knobs this process was started with.
    ///
    /// Two negative controls, one per direction: `pass` swaps a delta column, so a
    /// comparison that read only the header would not notice; `rung` moves a header word,
    /// so a comparison that read only the payload would not notice. Nothing else is
    /// mutable, and neither is reachable without a name in the environment.
    pub fn from_env() -> Knobs {
        Knobs {
            pass: match std::env::var("GM_MUTATE_GPU_FIXTURE_PASS").as_deref() {
                Ok("link") => Some("link"),
                Ok("collide") => Some("collide"),
                _ => None,
            },
            rung: std::env::var("GM_MUTATE_GPU_FIXTURE_RUNG")
                .ok()
                .and_then(|v| v.parse::<i32>().ok())
                .unwrap_or(0),
        }
    }
}

/// The deposit's fixed-point scale: the largest power of two that keeps `n` unit charges in
/// one cell under `i32::MAX`. `n = 1_000_000` gives `2^11`, so the quantum is 2^-11 of a unit
/// charge — the floor `docs/decisions/gpu-force-tier.md:88` states, reached from the code
/// rather than quoted from it.
///
/// Integer only, no `libm`: `32 - n.leading_zeros()` is `ceil(log2 n)`. `n = 0` is refused
/// rather than answered, because `ceil(log2 0)` has no value and the naive form would shift
/// by 31 and hand back a scale one charge in a cell could not hold — which is the one bound
/// this function exists to enforce.
pub fn scale_for(n: u32) -> Option<u32> {
    if n == 0 {
        return None;
    }
    Some(1u32 << (31 - (32 - n.leading_zeros() as i32)))
}

/// One case as the writer sees it: the probe's columns, the session's own positions, and
/// which state they are at. Everything that describes *what* is written, and nothing that
/// describes how — so `write` and `header` each stay inside the house limits without
/// either taking five arguments.
pub struct Case<'a> {
    /// The mesh's own columns at this state.
    pub probe: &'a MeshProbe,
    /// The session's `x` column, which the payload carries before the probe's own.
    pub xs: &'a [f64],
    /// The session's `y` column.
    pub ys: &'a [f64],
    /// Which of the two position sets these are.
    pub state: State,
}

/// One case's bytes: the 64-byte header, then every payload section in the wire order
/// `fixtures/gpu/README.md` states. Refuses rather than writes if any column holds a
/// non-finite value (D9).
pub fn write(case: &Case<'_>, knobs: &mut Knobs) -> Result<Vec<u8>, String> {
    let probe = case.probe;
    let (dx, dy) = columns(probe, knobs.pass);
    check_finite(case, &dx, &dy)?;
    let mut out = Vec::new();
    header(&mut out, case, knobs);
    put_u32(&mut out, probe.cells);
    put_u32(&mut out, probe.reach);
    put_u32s(&mut out, &probe.lo);
    put_u32s(&mut out, &probe.hi);
    put_f64s(&mut out, &probe.strength);
    put_f64s(&mut out, case.xs);
    put_f64s(&mut out, case.ys);
    put_f64s(&mut out, &probe.twiddle_re);
    put_f64s(&mut out, &probe.twiddle_im);
    put_f64s(&mut out, &probe.spectrum_re);
    put_f64s(&mut out, &probe.spectrum_im);
    put_f64s(&mut out, &probe.link_dx);
    put_f64s(&mut out, &probe.link_dy);
    put_f64s(&mut out, &dx);
    put_f64s(&mut out, &dy);
    put_f64s(&mut out, &probe.collide_dx);
    put_f64s(&mut out, &probe.collide_dy);
    Ok(out)
}

/// Every column this case would write, checked for finiteness before the first byte is put:
/// D9 says a fixture never carries a non-finite value, and one that did would be a silent
/// `NaN` in the arm's readback rather than a load error. `dx`/`dy` are the charge columns as
/// written, which under the `pass` control may be another pass's pair.
fn check_finite(case: &Case<'_>, dx: &[f64], dy: &[f64]) -> Result<(), String> {
    let probe = case.probe;
    finite("positions", case.xs.iter().chain(case.ys))?;
    for (name, column) in [
        ("strength", probe.strength.as_slice()),
        ("twiddles", probe.twiddle_re.as_slice()),
        ("twiddles", probe.twiddle_im.as_slice()),
        ("spectrum", probe.spectrum_re.as_slice()),
        ("spectrum", probe.spectrum_im.as_slice()),
        ("link", probe.link_dx.as_slice()),
        ("link", probe.link_dy.as_slice()),
        ("charge", dx),
        ("charge", dy),
        ("collide", probe.collide_dx.as_slice()),
        ("collide", probe.collide_dy.as_slice()),
    ] {
        finite(name, column.iter())?;
    }
    Ok(())
}

/// The columns written where the **charge** pair belongs: the charge pair, or — under the
/// `pass` negative control — another pass's pair, so a comparison that read only the header
/// would not notice.
fn columns(probe: &MeshProbe, pass: Option<&'static str>) -> (Vec<f64>, Vec<f64>) {
    match pass {
        Some("link") => (probe.link_dx.clone(), probe.link_dy.clone()),
        Some("collide") => (probe.collide_dx.clone(), probe.collide_dy.clone()),
        _ => (probe.charge_dx.clone(), probe.charge_dy.clone()),
    }
}

/// The 64-byte header: thirteen explicit words in wire order and no `#[repr(C)]`, because D6
/// makes a repr-C header a place a machine word can creep in, and the test
/// `every_integer_on_the_wire_is_u32` is a grep rather than a compile error.
fn header(out: &mut Vec<u8>, case: &Case<'_>, knobs: &Knobs) {
    let probe = case.probe;
    out.extend_from_slice(&MAGIC);
    put_u32(out, 1);
    put_u32(out, 0);
    put_u32(out, case.xs.len() as u32);
    put_u32(out, probe.lo.len() as u32);
    put_u32(out, probe.side);
    put_u32(out, case.state.word());
    put_u32(out, case.state.word());
    put_u32(out, rung_word(probe.step, knobs.rung));
    put_u32(out, 0);
    put_f64(out, probe.h);
    put_f64(out, probe.origin_x);
    put_f64(out, probe.origin_y);
    debug_assert_eq!(out.len(), HEADER_LEN, "the header is exactly 64 bytes");
}

/// The frame's rung on the wire: the two's complement of the `i32`, offset by whatever the
/// `rung` control asked for. An `as u32` is the two's complement by definition, which is
/// why the writer never spells the wrap out.
fn rung_word(step: i32, offset: i32) -> u32 {
    step.wrapping_add(offset) as u32
}

/// A refusal naming the column that holds a non-finite value: a fixture never carries one,
/// and one that did would be a silent `NaN` in the arm's readback rather than a load error.
fn finite<'a>(name: &str, column: impl Iterator<Item = &'a f64>) -> Result<(), String> {
    let mut column = column;
    match column.find(|v| !v.is_finite()) {
        Some(_) => Err(format!(
            "{name}: a column is not finite, and no fixture carries one"
        )),
        None => Ok(()),
    }
}

fn put_u32(out: &mut Vec<u8>, word: u32) {
    out.extend_from_slice(&word.to_le_bytes());
}

fn put_u32s(out: &mut Vec<u8>, words: &[u32]) {
    for word in words {
        put_u32(out, *word);
    }
}

fn put_f64(out: &mut Vec<u8>, value: f64) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn put_f64s(out: &mut Vec<u8>, values: &[f64]) {
    for value in values {
        put_f64(out, *value);
    }
}

// END OF THE WRITER: the test `every_integer_on_the_wire_is_u32` scans everything above
// this marker, so a width that reaches a field cannot reach a reader either.

#[cfg(test)]
fn u32_at(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().expect("four bytes"))
}

#[cfg(test)]
fn f64_at(bytes: &[u8], at: usize) -> f64 {
    f64::from_le_bytes(bytes[at..at + 8].try_into().expect("eight bytes"))
}
