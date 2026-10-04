//! The bodies the tests send: studio ingest documents, committed fixtures, failing and slow
//! streams.

use axum::body::{Body, Bytes};
use std::path::PathBuf;
use std::time::Duration;

/// A lower bound on `doc(n, m).len()`: every record at least as long as the first.
pub fn doc_floor((n, m): (u64, u64)) -> usize {
    let shortest = |count: u64, record: &str| {
        usize::try_from(count)
            .unwrap_or(usize::MAX)
            .saturating_mul(record.len())
    };
    let (node, edge) = (node(0), edge(2, 0));
    doc(0, 0)
        .len()
        .saturating_add(shortest(n, &node))
        .saturating_add(shortest(m, &edge))
}

/// A studio ingest document of `n` nodes and `m` edges, no loop and no repeated pair while
/// `m <= n * (n - 1) / 2` in the first `n / 2` offsets.
pub fn doc(n: usize, m: usize) -> String {
    let nodes: Vec<String> = (0..n).map(node).collect();
    let edges: Vec<String> = (0..m).map(|e| edge(n, e)).collect();
    format!(
        r#"{{"version":1,"nodes":[{}],"edges":[{}]}}"#,
        nodes.join(","),
        edges.join(",")
    )
}

fn node(i: usize) -> String {
    format!(
        r#"{{"id":"n{i}","kind":"record","database_id":null,"source":"t","label":"N{i}","group":null,"weight":1.0,"version":0.0,"has_note":false,"icon":null}}"#
    )
}

/// Edge `e` of a graph of `n` nodes.
fn edge(n: usize, e: usize) -> String {
    let (from, offset) = (e % n, 1 + e / n);
    let to = (from + offset) % n;
    format!(
        r#"{{"id":"e{e}","source":"n{from}","target":"n{to}","kind":"relation","label":"","strength":0.5,"directed":false,"record_id":null,"child_first":false}}"#
    )
}

/// A committed fixture, by its path under the repository root.
pub fn fixture(path: &str) -> Vec<u8> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    std::fs::read(root.join(path)).unwrap_or_else(|_| panic!("fixture {path}"))
}

/// A body whose first read is an error.
pub fn failing_body() -> Body {
    let chunk: Result<Bytes, std::io::Error> = Err(std::io::Error::other("read"));
    Body::from_stream(futures_util::stream::iter([chunk]))
}

/// A chunked body (no `Content-Length`) of `count` chunks of `size` bytes, `pause` apart.
pub fn chunked(size: usize, count: usize, pause: Duration) -> Body {
    let chunks = futures_util::stream::unfold(0, move |sent| async move {
        if sent == count {
            return None;
        }
        if !pause.is_zero() {
            tokio::time::sleep(pause).await;
        }
        let chunk: Result<Bytes, std::io::Error> = Ok(Bytes::from(vec![b' '; size]));
        Some((chunk, sent + 1))
    });
    Body::from_stream(chunks)
}
