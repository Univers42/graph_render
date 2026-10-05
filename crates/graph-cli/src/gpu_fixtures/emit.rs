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

/// Header length in bytes: ten `u32` words, one pad, then three `f64`s — no hole left open.
pub const HEADER_LEN: usize = 64;

/// The header's `u32` words in wire order, read back out of a written file. Test-only:
    /// `--check` compares bytes and never parses, so nothing in a run needs to read a
    /// header back — and the writer's own tests are the only place a reader exists.
    #[cfg(test)]
    pub fn header_words(bytes: &[u8]) -> Vec<u64> {
    (0..10).map(|w| u32_at(bytes, w * 4) as u64).collect()
}

/// The header's `f64` words in wire order: `h`, `origin_x`, `origin_y`. Test-only, as
    /// [`header_words`] is.
    #[cfg(test)]
    pub fn header_reals(bytes: &[u8]) -> Vec<f64> {
    (0..3).map(|w| f64_at(bytes, 40 + w * 8)).collect()
}

/// The deliberate wrongness `--check`'s negative controls switch on. Both default to off and
/// both are read from the environment once per run, so an ordinary run cannot reach them.
#[derive(Debug, Clone, Copy, Default)]
pub struct Knobs {
    /// Write the named pass's columns where the charge columns belong.
    pub pass: Option<&'static str>,
    /// Add this to the rung in the emitted header.
    pub rung: i64,
}

impl Knobs {
    /// The knobs an ordinary run has: none of them wrong. What the writer's own tests
    /// build, so a test never has to unset an inherited environment variable to be sure it
    /// is testing the honest writer.
    #[cfg(test)]
    pub fn none() -> Knobs {
        Knobs::default()
    }

    /// The knobs this process was started with, from the environment.
    ///
    /// Two negative controls, one per direction: `pass` swaps a delta column so the payload
    /// comparison has something to catch, and `rung` moves a header word so the header
    /// comparison has something to catch. Nothing else is mutable, and neither is reachable
    /// without a name in the environment.
    pub fn from_env() -> Knobs {
        let pass = match std::env::var("GM_MUTATE_GPU_FIXTURE_PASS").as_deref() {
            Ok("link") => Some("link"),
            Ok("collide") => Some("collide"),
            Ok(_) => None,
            _ => None,
        };
        let rung = std::env::var("GM_MUTATE_GPU_FIXTURE_RUNG")
            .ok()
            .and_then(|v| v.parse::<i64>().ok())
            .unwrap_or(0);
        Knobs { pass, rung }
    }
}

/// The deposit's fixed-point scale: the largest power of two that keeps `n` unit charges in
/// one cell under `i32::MAX`. `n = 1_000_000` gives `2^11`, so the quantum is 2^-11 of a unit
/// charge — the floor `docs/decisions/gpu-force-tier.md:88` states, reached from the code
/// rather than quoted from it.
///
/// Integer only, no `libm`: `32 - n.leading_zeros()` is `ceil(log2 n)`. `n = 0` is refused
/// rather than answered, because `ceil(log2 0)` has no value and the naive form would shift
/// by 31 and hand back a scale a cell of one charge could not hold — which is the one bound
/// this function exists to enforce.
pub fn scale_for(n: u32) -> Option<u32> {
    if n == 0 {
        return None;
    }
    Some(1u32 << (31 - (32 - n.leading_zeros() as i32)))
}

/// One case's bytes: the 64-byte header, then every payload section in the wire order
/// `fixtures/gpu/README.md` states.
pub fn write(
    probe: &MeshProbe,
    xs: &[f64],
    ys: &[f64],
    state: State,
    knobs: &mut Knobs,
) -> Result<Vec<u8>, String> {
    let n = xs.len() as u32;
    let m = probe.lo.len() as u32;
    check_finite("positions", xs.iter().chain(ys))?;
    check_finite("edge strengths", probe.strength.iter())?;
    let (dx, dy) = columns(probe, knobs.pass);
    for (name, column) in [
        ("link", &probe.link_dx),
        ("link", &probe.link_dy),
        ("charge", &dx),
        ("charge", &dy),
        ("collide", &probe.collide_dx),
        ("collide", &probe.collide_dy),
        ("twiddles", &probe.twiddle_re),
        ("twiddles", &probe.twiddle_im),
        ("spectrum", &probe.spectrum_re),
        ("spectrum", &probe.spectrum_im),
    ] {
        check_finite(name, column.iter())?;
    }
    let mut out = Vec::with_capacity(64 + 8 + 16 * m as usize + 64 * n as usize);
    header(&mut out, probe, n, m, state, knobs);
    put_u32(&mut out, probe.cells);
    put_u32(&mut out, probe.reach);
    put_u32s(&mut out, &probe.lo);
    put_u32s(&mut out, &probe.hi);
    put_f64s(&mut out, &probe.strength);
    put_f64s(&mut out, xs);
    put_f64s(&mut out, ys);
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

/// The columns written where the **charge** pair belongs: the charge pair, or — under the
/// `pass` negative control — another pass's pair, so a comparison that only read the header
/// would not notice.
fn columns(probe: &MeshProbe, pass: Option<&'static str>) -> (Vec<f64>, Vec<f64>) {
    match pass {
        Some("link") => (probe.link_dx.clone(), probe.link_dy.clone()),
        Some("collide") => (probe.collide_dx.clone(), probe.collide_dy.clone()),
        _ => (probe.charge_dx.clone(), probe.charge_dy.clone()),
    }
}

/// The 64-byte header, sixteen explicit words in wire order and no `#[repr(C)]`: D6 makes a
/// repr-C header a place a machine word can creep in, and the test
/// `every_integer_on_the_wire_is_u32` is a grep rather than a compile error.
fn header(out: &mut Vec<u8>, probe: &MeshProbe, n: u32, m: u32, state: State, knobs: &Knobs) {
    out.extend_from_slice(&MAGIC);
    put_u32(out, 1);
    put_u32(out, 0);
    put_u32(out, n);
    put_u32(out, m);
    put_u32(out, probe.side);
    put_u32(out, state.word());
    put_u32(out, state.word());
    put_u32(out, (i128::from(probe.step) + i128::from(knobs.rung)) as i64 as u32);
    put_u32(out, 0);
    put_f64(out, probe.h);
    put_f64(out, probe.origin_x);
    put_f64(out, probe.origin_y);
    debug_assert_eq!(out.len(), HEADER_LEN, "the header is exactly 64 bytes");
}

/// A refusal for the first non-finite value in `column`, naming the column: D9 says a
/// fixture never carries one, and a fixture that did would be a silent `NaN` in the arm's
/// readback rather than a load error.
fn check_finite<'a>(name: &str, column: impl Iterator<Item = &'a f64>) -> Result<(), String> {
    match column.into_iter().find(|v| !v.is_finite()) {
        Some(_) => Err(format!("{name}: a column is not finite, and no fixture carries one")),
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

#[cfg(test)]
fn u32_at(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().expect("four bytes"))
}

#[cfg(test)]
fn f64_at(bytes: &[u8], at: usize) -> f64 {
    f64::from_le_bytes(bytes[at..at + 8].try_into().expect("eight bytes"))
}
