//! D1, measured rather than asserted (`prompt.md` §6). The sweep and the evaluation
//! live in `graph_wasm::probe`; this runs that one piece of code natively and as
//! wasm32 under Node, then compares the four bit-pattern pairs that matter.

use crate::hashgate::{build_wasm, file_sha256, node_harness, run_lines, workspace_root};
use graph_wasm::probe::{FUNCTIONS, RANDOM_PER_FUNCTION, SPECIALS};
use std::path::Path;
use std::process::{Command, ExitCode};

/// One probe record: `x`, `y`, `std(x, y)`, `libm(x, y)` as bit patterns.
type Record = [u64; 4];

/// Differing-bit-pattern counts for one function.
#[derive(Debug, Default, PartialEq, Eq)]
struct Counts {
    inputs: usize,
    native_std_vs_libm: usize,
    wasm_std_vs_libm: usize,
    std_native_vs_wasm: usize,
    libm_native_vs_wasm: usize,
    max_ulp_std_native_vs_wasm: u64,
}

/// Runs the probe on both targets and writes the measurement to `out` (relative to the root).
pub fn run(out: &Path) -> ExitCode {
    match measure().and_then(|doc| write(out, &doc)) {
        Ok(summary) => {
            println!("{summary}");
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("determinism-probe: {err}");
            ExitCode::from(2)
        }
    }
}

fn measure() -> Result<String, String> {
    let native = parse(&graph_wasm::probe::probe_bytes())?;
    let wasm_path = build_wasm()?;
    let hex = run_lines(node_harness(&wasm_path).arg("probe"))?.concat();
    let wasm = parse(&decode_hex(&hex)?)?;
    let counts = native
        .iter()
        .zip(&wasm)
        .map(|(n, w)| compare(n, w))
        .collect::<Result<Vec<_>, _>>()?;
    let tools = format!(
        "{} · node {} · native libm is {}",
        tool_version("rustc")?,
        tool_version("node")?,
        tool_version("ldd")?
    );
    Ok(render(&counts, &tools, &file_sha256(&wasm_path)?))
}

fn write(out: &Path, doc: &str) -> Result<String, String> {
    let path = workspace_root().join(out);
    std::fs::write(&path, doc).map_err(|e| format!("writing {}: {e}", path.display()))?;
    let table = doc
        .lines()
        .filter(|l| l.starts_with('|'))
        .collect::<Vec<_>>()
        .join("\n");
    Ok(format!("{table}\nwrote {}", out.display()))
}

fn parse(bytes: &[u8]) -> Result<Vec<Vec<Record>>, String> {
    let functions = u32_at(bytes, 0)?;
    let mut at = 4;
    let mut all = Vec::with_capacity(functions);
    for _ in 0..functions {
        let count = u32_at(bytes, at)?;
        let body = bytes
            .get(at + 4..at + 4 + count * 32)
            .ok_or("truncated probe records")?;
        at += 4 + count * 32;
        let word = |i: usize| u64::from_le_bytes(core::array::from_fn(|k| body[i * 8 + k]));
        all.push(
            (0..count)
                .map(|r| core::array::from_fn(|k| word(r * 4 + k)))
                .collect(),
        );
    }
    Ok(all)
}

fn u32_at(bytes: &[u8], at: usize) -> Result<usize, String> {
    let word = bytes
        .get(at..at + 4)
        .ok_or_else(|| format!("probe buffer truncated at byte {at}"))?;
    Ok(u32::from_le_bytes(core::array::from_fn(|k| word[k])) as usize)
}

fn compare(native: &[Record], wasm: &[Record]) -> Result<Counts, String> {
    if native.len() != wasm.len() || native.iter().zip(wasm).any(|(n, w)| n[..2] != w[..2]) {
        return Err(
            "native and wasm32 swept different inputs; the probe itself is not deterministic"
                .into(),
        );
    }
    let mut counts = Counts {
        inputs: native.len(),
        ..Counts::default()
    };
    for (n, w) in native.iter().zip(wasm) {
        counts.native_std_vs_libm += usize::from(n[2] != n[3]);
        counts.wasm_std_vs_libm += usize::from(w[2] != w[3]);
        counts.std_native_vs_wasm += usize::from(n[2] != w[2]);
        counts.libm_native_vs_wasm += usize::from(n[3] != w[3]);
        counts.max_ulp_std_native_vs_wasm = counts.max_ulp_std_native_vs_wasm.max(ulps(n[2], w[2]));
    }
    Ok(counts)
}

/// Distance in units in the last place between two doubles given as bits.
fn ulps(a: u64, b: u64) -> u64 {
    let ordered = |bits: u64| {
        if bits >> 63 == 1 {
            -((bits & !(1 << 63)) as i128)
        } else {
            bits as i128
        }
    };
    (ordered(a) - ordered(b)).unsigned_abs() as u64
}

fn decode_hex(hex: &str) -> Result<Vec<u8>, String> {
    let digits = hex.trim().as_bytes();
    if !digits.len().is_multiple_of(2) {
        return Err("odd-length hex from the wasm arm".into());
    }
    let nibble = |c: u8| {
        (c as char)
            .to_digit(16)
            .ok_or_else(|| format!("bad hex digit {:?}", c as char))
    };
    let (pairs, _) = digits.as_chunks::<2>();
    pairs
        .iter()
        .map(|&[hi, lo]| Ok((nibble(hi)? * 16 + nibble(lo)?) as u8))
        .collect()
}

fn tool_version(tool: &str) -> Result<String, String> {
    let lines = run_lines(Command::new(tool).arg("--version"))?;
    Ok(lines.first().map_or("unknown", |l| l.trim()).to_owned())
}

const REPORT: &str = r#"# D1 — `std` vs `libm` transcendentals, native vs wasm32

Generated by `graph-cli determinism-probe`; do not edit by hand. Re-run to refresh:

```sh
docker run --rm -v "$PWD:/w" ge-rust cargo run -p graph-cli -- determinism-probe
```

Toolchain: {tools}. wasm32 artifact `graph_wasm.wasm` sha256 `{wasm_sha}`.

## The sweep

Defined once in `crates/graph-wasm/src/lib.rs` (`mod probe`) and executed unchanged on both
targets: natively inside `graph-cli`, and as wasm32 through `harness/wasm-run.mjs` under Node. The
two runs are checked to have swept bit-identical inputs before any output is compared.

- **Specials** ({specials} magnitudes, each with its negation, plus ±0): denormals (`5e-324`,
  `1e-320`, `2.225e-309`, the largest subnormal), `1e-300` … `0.5`, values around 1.0
  (`0.999999`, `1 − ε`, `1`, `1 + ε`, `1.000001`), and large magnitudes up to `f64::MAX`.
  Each function keeps only the specials inside its domain (`ln_1p`: x > −1; `ln`: x > 0).
  `pow` pairs each magnitude with exponents `0.5`, `−3`, `40`, and a negative base with `3`;
  `atan2` pairs every signed special with `±1` and `±0`.
- **Seeded pseudo-random inputs**: {random} per function from xorshift64 seeded
  `0x5EED_D1D1_0000_0001 ^ function index`, built from raw bit patterns so whole binades are
  covered: `ln_1p` over (−1, 0) and binades 2⁻⁶⁰…2⁶⁰; `ln` over every normal binade; `exp`
  uniform on [−745, 709]; `sin`/`cos` signed, binades 2⁻³⁰…2³⁰; `pow` base in 2⁻³⁰…2³⁰ with
  exponent uniform on [−40, 40]; `atan2` both arguments signed in 2⁻³⁰…2³⁰.

## Differing bit patterns

{table}
- *native std≠libm* — on the native target, `std` and `libm` disagree.
- *wasm32 std≠libm* — the same, on wasm32.
- *std native≠wasm32* — the claim D1 was written on: `std` on one target against `std` on the other.
- *libm native≠wasm32* — the property D1 relies on; any non-zero value here is a stop.

## Conclusion

{conclusion}

## Limits

Ponytail: this is a sampler, not the whole f64 domain. An input on which the implementations round
differently but that the sweep never visits goes uncounted, so the probe can only **under-report**
divergence — the dangerous direction. Escape hatch: raise `RANDOM_PER_FUNCTION` or add the suspect
input to `SPECIALS`. The sweep is f64 only; the `f32` variants (`log1pf`, …) are not probed.
"#;

fn render(counts: &[Counts], tools: &str, wasm_sha: &str) -> String {
    let mut table = String::from(
        "| function | inputs | native std≠libm | wasm32 std≠libm | std native≠wasm32 | libm native≠wasm32 | max ULP std native↔wasm32 |\n|---|---:|---:|---:|---:|---:|---:|\n",
    );
    for (name, c) in FUNCTIONS.iter().zip(counts) {
        table.push_str(&format!(
            "| `{name}` | {} | {} | {} | {} | {} | {} |\n",
            c.inputs,
            c.native_std_vs_libm,
            c.wasm_std_vs_libm,
            c.std_native_vs_wasm,
            c.libm_native_vs_wasm,
            c.max_ulp_std_native_vs_wasm
        ));
    }
    let std_split: usize = counts.iter().map(|c| c.std_native_vs_wasm).sum();
    let libm_split: usize = counts.iter().map(|c| c.libm_native_vs_wasm).sum();
    [
        ("{tools}", tools.to_owned()),
        ("{wasm_sha}", wasm_sha.to_owned()),
        ("{specials}", SPECIALS.len().to_string()),
        ("{random}", RANDOM_PER_FUNCTION.to_string()),
        ("{table}", table),
        ("{conclusion}", conclusion(std_split, libm_split)),
    ]
    .into_iter()
    .fold(REPORT.to_owned(), |doc, (key, value)| {
        doc.replace(key, &value)
    })
}

fn conclusion(std_split: usize, libm_split: usize) -> String {
    let libm = if libm_split == 0 {
        "`libm` is bit-identical between native and wasm32 on every input of the sweep, which is what D1 relies on.".to_string()
    } else {
        format!(
            "**`libm` itself differs between native and wasm32 on {libm_split} inputs. D1 as written is insufficient — stop and report.**"
        )
    };
    let std = if std_split == 0 {
        "`std` also agreed across targets on this sweep, so D1 is **cheap insurance**, not a measured necessity: the constraint stands, with this measurement as its recorded reason.".to_string()
    } else {
        format!(
            "`std` differs between native and wasm32 on **{std_split}** inputs, so D1 is **confirmed by measurement**: a motor calling `std` transcendentals would hash differently on the two targets."
        )
    };
    format!("{libm}\n\n{std}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ulps_counts_across_zero_and_between_neighbours() {
        assert_eq!(ulps(1.0f64.to_bits(), 1.0f64.to_bits()), 0);
        assert_eq!(ulps(1.0f64.to_bits(), (1.0f64.to_bits()) + 1), 1);
        assert_eq!(ulps(0.0f64.to_bits(), (-0.0f64).to_bits()), 0);
        assert_eq!(
            ulps(f64::from_bits(1).to_bits(), (-f64::from_bits(1)).to_bits()),
            2
        );
    }

    #[test]
    fn native_buffer_parses_back_into_every_function() {
        let parsed = parse(&graph_wasm::probe::probe_bytes()).expect("parses");
        assert_eq!(parsed.len(), FUNCTIONS.len());
        let counts = compare(&parsed[0], &parsed[0]).expect("same inputs");
        assert_eq!(
            (counts.std_native_vs_wasm, counts.libm_native_vs_wasm),
            (0, 0)
        );
    }

    #[test]
    fn hex_round_trips_and_rejects_garbage() {
        assert_eq!(decode_hex("00ff10"), Ok(vec![0, 255, 16]));
        assert!(decode_hex("0").is_err() && decode_hex("zz").is_err());
    }
}
