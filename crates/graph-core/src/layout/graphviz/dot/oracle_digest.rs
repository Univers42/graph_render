//! What `harness/oracle-dot-probe.py` wrote: one row per fixture seed, holding the oracle's
//! rank for every node and the oracle's order within every rank.
//!
//! The probe is the only thing that can produce this, and it runs inside the oracle image;
//! [`oracle_digest`] reads what it left in `target/probe/dot1000.txt` and is the reason both
//! sweeps are `#[ignore]`d — a clean checkout has no `target/`. The file's line format is the
//! probe's, and the parse is strict: a field that is not a number is a panic, not a zero,
//! because a silently miscounted row would be a silently wrong measurement.

/// One row of `target/probe/dot1000.txt`: the oracle's answer for one seed.
pub struct OracleRow {
    /// The fixture edges, as (tail, head), in declaration order.
    pub edges: Vec<(u32, u32)>,
    /// The rank the oracle gave every node.
    pub ranks: Vec<i32>,
    /// The nodes of every rank left to right, rank 0 first: a permutation of `ranks.len()`
    /// node indices, which is what the probe's order column is.
    pub order: Vec<u32>,
}

impl OracleRow {
    /// The oracle's order as one row per rank, so it can be compared with
    /// [`crossings::rank_rows`] without the caller regrouping it.
    pub fn rows(&self) -> Vec<Vec<u32>> {
        let highest = self.ranks.iter().copied().max().unwrap_or(-1);
        let mut rows = vec![Vec::new(); usize::try_from(highest + 1).expect("a rank count")];
        for &node in &self.order {
            rows[usize::try_from(self.ranks[node as usize]).expect("a rank")].push(node);
        }
        rows
    }
}

/// The oracle's per-seed layering and order, as `harness/oracle-dot-probe.py --digest` wrote
/// it: `seed n t,h ... <n ranks> <n order>`, one line per seed.
///
/// The file is a probe output under `target/`, so it is absent from a clean checkout; the two
/// tests that read it are `#[ignore]`d for that reason. It is produced by
/// `python3 harness/oracle-dot-probe.py target/dotfix --fixtures=dot.jsonl --digest
/// target/probe/dot1000.txt` inside `ge-graphviz-oracle`, from the same fixture set as the
/// twenty seeds in `rank_tests.rs`.
pub fn oracle_digest() -> Vec<OracleRow> {
    let path = digest_path();
    let Ok(text) = std::fs::read_to_string(&path) else {
        panic!(
            "{} is missing; see this module's doc for the command that writes it",
            path.display()
        );
    };
    let rows: Vec<OracleRow> = text
        .lines()
        .filter(|l| !l.starts_with('#'))
        .map(parse_row)
        .collect();
    assert!(!rows.is_empty(), "the digest has rows");
    rows
}

/// Where the probe writes its rows: under `target/`, so a clean checkout has none.
fn digest_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/probe/dot1000.txt")
}

/// One digest line as an [`OracleRow`]: `seed n`, the edges, the ranks, then the order.
fn parse_row(line: &str) -> OracleRow {
    let mut fields = line.split_whitespace();
    let _seed: u32 = fields.next().expect("a seed").parse().expect("a seed");
    let count: usize = fields
        .next()
        .expect("a node count")
        .parse()
        .expect("a count");
    let rest: Vec<&str> = fields.collect();
    let (pairs, tail) = rest.split_at(rest.len() - 2 * count);
    let numbers = |slice: &[&str]| -> Vec<i32> {
        slice.iter().map(|n| n.parse().expect("a number")).collect()
    };
    OracleRow {
        edges: pairs
            .iter()
            .map(|pair| {
                let (tail, head) = pair.split_once(',').expect("a tail,head pair");
                (tail.parse().expect("a tail"), head.parse().expect("a head"))
            })
            .collect(),
        ranks: numbers(&tail[..count]),
        order: numbers(&tail[count..])
            .into_iter()
            .map(|n| n as u32)
            .collect(),
    }
}
