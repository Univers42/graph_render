//! The reader's cost on a long document. Every record once named an earlier one by a scan
//! of the records before it (the duplicate check, then each `parent` and `link` cell), so
//! a 64 MiB body took over 20 minutes to read; the indexes `validate` builds once per
//! document make the read `O(n log n)`.
//!
//! Caveat: a wall-clock bound, not an operation count. Failing input: a host so loaded that
//! reading [`N`] records (a few seconds in a debug build here) takes [`BOUND`]; the scans
//! made about 5·10^10 record comparisons on the same document. Direction: a false red,
//! never a false green. Escape hatch: re-run the test on its own.

use super::support::*;
use std::fmt::Write;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

const BOUND: Duration = Duration::from_secs(20);
const N: usize = 200_000;

const HEAD: &str = r#"{"version":1,"source":"rows","collections":[{"id":"task","name":"Tasks","titleField":"name","fields":[{"id":"name","name":"Name","role":"title","link":null},{"id":"up","name":"Up","role":"parent","link":null},{"id":"blocks","name":"Blocks","role":"link","link":{"collection":"task","cardinality":"many","symmetric":false}}]}],"records":["#;

/// `read(text)` on its own thread, failing the test when it has not returned within
/// [`BOUND`]. The thread is left behind on a failure and dies with the test process.
fn read_within_bound(text: String) -> Result<usize, String> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let outcome = read(&text).map(|doc| doc.records.len());
        sender.send(outcome.map_err(|e| e.to_string()))
    });
    receiver
        .recv_timeout(BOUND)
        .unwrap_or_else(|_| panic!("not read within {BOUND:?}: a check is super-linear"))
}

/// `N` records, each but the first naming two others (an earlier parent and a link to a
/// record anywhere in the document), then `tail`.
fn document(tail: &str) -> String {
    let mut text = String::from(HEAD);
    for i in 0..N {
        let refs = match i {
            0 => String::new(),
            _ => format!(r#","up":"r{}","blocks":["r{}"]"#, (i - 1) / 2, i * 7919 % N),
        };
        write!(
            text,
            r#"{{"id":"r{i}","collection":"task","deleted":false,"updatedAt":{i},"values":{{"name":"Task {i}"{refs}}}}},"#
        )
        .expect("writing to a String does not fail");
    }
    text.push_str(tail);
    text.push_str("]}");
    text
}

#[test]
fn a_long_document_reads_within_the_bound() {
    let last =
        r#"{"id":"last","collection":"task","deleted":false,"updatedAt":0,"values":{"up":"r0"}}"#;
    assert_eq!(read_within_bound(document(last)), Ok(N + 1));
}

#[test]
fn a_repeat_at_the_end_of_a_long_document_names_its_own_index() {
    let repeat = r#"{"id":"r0","collection":"task","deleted":false,"updatedAt":0,"values":{}}"#;
    assert_eq!(
        read_within_bound(document(repeat)),
        Err(format!("records[{N}]: duplicate record id `r0`"))
    );
}
