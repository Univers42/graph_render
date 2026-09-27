//! D1, measured rather than asserted (`prompt.md` §6). The sweep and the evaluation
//! live in `graph_wasm::probe`; this runs that one piece of code natively and as
//! wasm32 under Node, then compares the four bit-pattern pairs that matter.

use crate::probe_report::{Counts, render};
use crate::runner::{build_wasm, file_sha256, node_harness, run_lines, workspace_root};
use std::path::Path;
use std::process::{Command, ExitCode};

/// One probe record: `x`, `y`, `std(x, y)`, `libm(x, y)` as bit patterns.
type Record = [u64; 4];

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
    let wasm_path = build_wasm(&["probe"])?;
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
        assert_eq!(parsed.len(), graph_wasm::probe::FUNCTIONS.len());
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

    fn record(x: u64, std: u64, libm: u64) -> Record {
        [x, 0, std, libm]
    }

    #[test]
    fn compare_counts_each_kind_of_split_separately() {
        let native = [record(1, 10, 10), record(2, 20, 21), record(3, 30, 30)];
        let wasm = [record(1, 10, 10), record(2, 21, 21), record(3, 33, 31)];
        let counts = compare(&native, &wasm).expect("same inputs");
        let want = Counts {
            inputs: 3,
            native_std_vs_libm: 1,
            wasm_std_vs_libm: 1,
            std_native_vs_wasm: 2,
            libm_native_vs_wasm: 1,
            max_ulp_std_native_vs_wasm: 3,
        };
        assert_eq!(counts, want);
    }

    #[test]
    fn compare_refuses_inputs_that_differ_between_targets() {
        let native = [record(1, 0, 0), record(2, 0, 0)];
        assert!(compare(&native, &native[..1]).is_err());
        assert!(compare(&native, &[record(1, 0, 0), record(9, 0, 0)]).is_err());
        let mut y_differs = native;
        y_differs[1][1] = 5;
        assert!(compare(&native, &y_differs).is_err());
    }

    #[test]
    fn parse_reads_two_functions_and_refuses_truncation() {
        let mut bytes = 2u32.to_le_bytes().to_vec();
        for (count, base) in [(1u32, 10u64), (2, 20)] {
            bytes.extend_from_slice(&count.to_le_bytes());
            for k in 0..u64::from(count) * 4 {
                bytes.extend_from_slice(&(base + k).to_le_bytes());
            }
        }
        let parsed = parse(&bytes).expect("well-formed");
        assert_eq!(
            parsed,
            [
                vec![[10, 11, 12, 13]],
                vec![[20, 21, 22, 23], [24, 25, 26, 27]]
            ]
        );
        assert!(parse(&bytes[..bytes.len() - 1]).is_err());
        assert!(parse(&bytes[..2]).is_err());
    }
}
