use super::*;

/// FNV-1a over the little-endian bytes of `words`.
fn fnv1a(words: impl IntoIterator<Item = u64>) -> u64 {
    words
        .into_iter()
        .flat_map(u64::to_le_bytes)
        .fold(0xCBF2_9CE4_8422_2325, |h, b| {
            (h ^ u64::from(b)).wrapping_mul(0x100_0000_01B3)
        })
}

#[test]
fn sweep_is_fixed_and_stays_in_each_domain() {
    assert_eq!(probe_bytes(), probe_bytes());
    for (x, _) in inputs(0) {
        assert!(x > -1.0, "ln_1p input {x} outside its domain");
    }
    for (x, _) in inputs(1) {
        assert!(x > 0.0, "ln input {x} outside its domain");
    }
}

#[test]
fn sweep_covers_denormals_and_both_signs() {
    let sines = inputs(3);
    assert!(
        sines
            .iter()
            .any(|&(x, _)| x != 0.0 && x.abs() < f64::MIN_POSITIVE)
    );
    assert!(sines.iter().any(|&(x, _)| x < 0.0) && sines.iter().any(|&(x, _)| x > 0.0));
}

/// The sweep is the one `docs/measurements/d1-ln1p.md` was measured on. Inputs are
/// built from integer bit operations and IEEE arithmetic only, so this pin holds on
/// every target. Changing the sweep is allowed; doing it silently is not — update the
/// pin and regenerate the measurement together.
#[test]
fn sweep_inputs_are_pinned() {
    let counts: Vec<usize> = (0..FUNCTIONS.len()).map(|f| inputs(f).len()).collect();
    assert_eq!(counts, [2088, 2074, 2102, 2102, 2102, 2152, 2264]);
    let words = (0..FUNCTIONS.len())
        .flat_map(inputs)
        .flat_map(|(x, y)| [x.to_bits(), y.to_bits()]);
    assert_eq!(fnv1a(words), 0x9214_4143_699D_208E);
}

#[test]
fn random_inputs_span_each_declared_range() {
    let random = |f: usize| inputs(f).split_off(inputs(f).len() - RANDOM_PER_FUNCTION as usize);
    let ln_1p = random(0);
    assert!(ln_1p.iter().any(|&(x, _)| (-1.0..0.0).contains(&x)));
    assert!(ln_1p.iter().any(|&(x, _)| x > 1.0));
    assert!(
        random(2)
            .iter()
            .all(|&(x, _)| (-745.0..=709.0).contains(&x))
    );
    let pow = random(5);
    assert!(
        pow.iter()
            .all(|&(x, y)| x > 0.0 && (-40.0..=40.0).contains(&y))
    );
    assert!(pow.iter().any(|&(_, y)| y < -20.0) && pow.iter().any(|&(_, y)| y > 20.0));
}

#[test]
fn bit_helpers_do_what_their_names_say() {
    for bits in [0, u64::MAX, 0x0123_4567_89AB_CDEF] {
        let exponent = from_bits(bits, 993, 1053).to_bits() >> 52;
        assert!((993..=1053).contains(&exponent), "{exponent}");
        assert!((0.0..1.0).contains(&unit(bits)));
    }
    assert_eq!(unit(0), 0.0);
    assert!(unit(u64::MAX) > 0.999_999);
    assert_eq!((signed(0, 2.0), signed(1 << 63, 2.0)), (2.0, -2.0));
    let mut state = 0x0123_4567_89AB_CDEF;
    assert_eq!(next(&mut state), 0x3F28_00D6_569E_01B4);
    assert_eq!(next(&mut state), 0x606F_949A_3CEB_D0B7);
}

#[test]
fn evaluate_maps_each_index_to_its_named_function() {
    let (x, y): (f64, f64) = (0.5, 0.25);
    let expected = [
        (x.ln_1p(), libm::log1p(x)),
        (x.ln(), libm::log(x)),
        (x.exp(), libm::exp(x)),
        (x.sin(), libm::sin(x)),
        (x.cos(), libm::cos(x)),
        (x.powf(y), libm::pow(x, y)),
        (x.atan2(y), libm::atan2(x, y)),
    ];
    for (f, want) in expected.into_iter().enumerate() {
        assert_eq!(evaluate(f, x, y), want, "{}", FUNCTIONS[f]);
    }
}

#[test]
fn probe_bytes_frames_every_function_and_record() {
    let bytes = probe_bytes();
    let word = |at: usize| u32::from_le_bytes(core::array::from_fn(|k| bytes[at + k]));
    assert_eq!(word(0), FUNCTIONS.len() as u32);
    let mut at = 4;
    for f in 0..FUNCTIONS.len() {
        let inputs = inputs(f);
        assert_eq!(word(at) as usize, inputs.len());
        let (x, y) = inputs[0];
        let (via_std, via_libm) = evaluate(f, x, y);
        let first: Vec<u8> = [x, y, via_std, via_libm]
            .iter()
            .flat_map(|v| v.to_bits().to_le_bytes())
            .collect();
        assert_eq!(&bytes[at + 4..at + 36], first.as_slice());
        at += 4 + 32 * inputs.len();
    }
    assert_eq!(at, bytes.len());
}
