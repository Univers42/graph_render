//! The D1 measurement: `std` against `libm`, bit for bit, over a fixed input sweep.
//!
//! Ponytail: this is a sampler over a chosen sweep (fixed specials plus 2048 seeded
//! pseudo-random inputs per function), not the whole f64 domain. An input where `std`
//! and `libm` round differently but that the sweep never visits goes uncounted, so
//! the probe can only **under-report** divergence — the dangerous direction. Escape
//! hatch: raise `RANDOM_PER_FUNCTION` or add the suspect input to `SPECIALS`; the
//! libm constraint (D1) stands regardless of what this probe reports.

/// Function names, in buffer order.
pub const FUNCTIONS: [&str; 7] = ["ln_1p", "ln", "exp", "sin", "cos", "pow", "atan2"];
/// Seeded pseudo-random inputs per function, after the specials.
pub const RANDOM_PER_FUNCTION: u32 = 2048;
const SEED: u64 = 0x5EED_D1D1_0000_0001;
/// Denormals, values around 1.0, small and large magnitudes. Negated copies are added.
pub const SPECIALS: [f64; 26] = [
    5e-324,
    1e-320,
    2.225e-309,
    2.225_073_858_507_201e-308,
    1e-300,
    1e-100,
    1e-10,
    1e-5,
    0.1,
    0.5,
    0.999_999,
    1.0 - f64::EPSILON,
    1.0,
    1.0 + f64::EPSILON,
    1.000_001,
    2.0,
    core::f64::consts::PI,
    10.0,
    100.0,
    1e5,
    1e10,
    1e22,
    1e100,
    1e300,
    1.7e308,
    f64::MAX,
];

/// The `(x, y)` inputs for function index `function`; `y` is unused by unary functions.
pub fn inputs(function: usize) -> Vec<(f64, f64)> {
    let signed = SPECIALS.iter().flat_map(|&v| [v, -v]).chain([0.0, -0.0]);
    let mut all: Vec<(f64, f64)> = match function {
        5 => SPECIALS
            .iter()
            .flat_map(|&x| [(x, 0.5), (x, -3.0), (-x, 3.0), (x, 40.0)])
            .collect(),
        6 => signed
            .clone()
            .flat_map(|y| [(y, 1.0), (y, -1.0), (y, 0.0), (y, -0.0)])
            .collect(),
        _ => signed
            .map(|x| (x, 0.0))
            .filter(|&(x, _)| in_domain(function, x))
            .collect(),
    };
    let mut state = SEED ^ (function as u64);
    all.extend((0..RANDOM_PER_FUNCTION).map(|_| random_input(function, &mut state)));
    all
}

fn in_domain(function: usize, x: f64) -> bool {
    match function {
        0 => x > -1.0,
        1 => x > 0.0,
        _ => true,
    }
}

fn random_input(function: usize, state: &mut u64) -> (f64, f64) {
    let (a, b) = (next(state), next(state));
    match function {
        0 if a & 1 == 0 => (-from_bits(a, 1023 - 60, 1022), 0.0),
        0 => (from_bits(a, 1023 - 60, 1023 + 60), 0.0),
        1 => (from_bits(a, 1, 2046), 0.0),
        2 => (-745.0 + 1454.0 * unit(a), 0.0),
        3 | 4 => (signed(a, from_bits(a, 1023 - 30, 1023 + 30)), 0.0),
        5 => (from_bits(a, 1023 - 30, 1023 + 30), -40.0 + 80.0 * unit(b)),
        _ => (
            signed(a, from_bits(a, 993, 1053)),
            signed(b, from_bits(b, 993, 1053)),
        ),
    }
}

/// A positive double whose biased exponent lies in `[lo, hi]`, mantissa from `bits`.
fn from_bits(bits: u64, lo: u64, hi: u64) -> f64 {
    let exponent = lo + (bits >> 52) % (hi - lo + 1);
    f64::from_bits((exponent << 52) | (bits & ((1 << 52) - 1)))
}

fn signed(bits: u64, magnitude: f64) -> f64 {
    if bits & (1 << 63) == 0 {
        magnitude
    } else {
        -magnitude
    }
}

fn unit(bits: u64) -> f64 {
    (bits >> 11) as f64 / (1u64 << 53) as f64
}

fn next(state: &mut u64) -> u64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}

/// `(std, libm)` for function index `function` at `(x, y)`.
pub fn evaluate(function: usize, x: f64, y: f64) -> (f64, f64) {
    match function {
        0 => (x.ln_1p(), libm::log1p(x)),
        1 => (x.ln(), libm::log(x)),
        2 => (x.exp(), libm::exp(x)),
        3 => (x.sin(), libm::sin(x)),
        4 => (x.cos(), libm::cos(x)),
        5 => (x.powf(y), libm::pow(x, y)),
        _ => (x.atan2(y), libm::atan2(x, y)),
    }
}

/// `[fn count: u32]` then per function `[count: u32]` and `count` records of four
/// little-endian `u64` bit patterns: `x`, `y`, `std(x, y)`, `libm(x, y)`.
pub fn probe_bytes() -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&(FUNCTIONS.len() as u32).to_le_bytes());
    for function in 0..FUNCTIONS.len() {
        let inputs = inputs(function);
        out.extend_from_slice(&(inputs.len() as u32).to_le_bytes());
        for (x, y) in inputs {
            let (via_std, via_libm) = evaluate(function, x, y);
            for value in [x, y, via_std, via_libm] {
                out.extend_from_slice(&value.to_bits().to_le_bytes());
            }
        }
    }
    out
}

#[cfg(test)]
mod tests;
