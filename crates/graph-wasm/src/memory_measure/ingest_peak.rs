//! The ingest term of the HTTP service's per-slot memory (`docs/measurements/service-caps.md`):
//! the peak heap each `source=` reader holds over one body near the service's 64 MiB limit,
//! through the same calls the server makes (`docs/contract/service-api.md`): studio is
//! [`crate::ingest::read_records`] then [`crate::ingest::index`], contract is
//! [`crate::contract::derive`]. The body text itself is built before the count starts, so
//! it is excluded, as the server counts the body separately.
//!
//! ```sh
//! scripts/orch/gr cargo test --release -p graph-wasm --lib -- --ignored --nocapture ingest_peak
//! ```
//!
//! Caveat: the documents are synthetic. The studio one is the seeded model (m ≈ 1.55 n); the
//! contract one is one collection with a parent, a tag and one link per record. A body of
//! the same size made of shorter records holds more of them, and its peak can be higher.

use super::measure;
use std::fmt::Write;

const BODY: usize = 64 << 20;

const CONTRACT_HEAD: &str = r#"{"version":1,"source":"rows","collections":[{"id":"task","name":"Tasks","titleField":"name","fields":[{"id":"name","name":"Name","role":"title","link":null},{"id":"up","name":"Up","role":"parent","link":null},{"id":"labels","name":"Labels","role":"tags","link":null},{"id":"effort","name":"Effort","role":"weight","link":null},{"id":"blocks","name":"Blocks","role":"link","link":{"collection":"task","cardinality":"many","symmetric":false}}]}],"records":["#;

#[test]
#[ignore = "a measurement, not a check: run alone with --release -- --ignored --nocapture"]
fn ingest_peak_at_the_service_body_limit() {
    println!("| source | body bytes | nodes | heap peak bytes | resident rise bytes |");
    println!("|---|---|---|---|---|");
    let text = studio_document(BODY);
    let ((nodes, heap), rise) = resident_rise(|| {
        measure(|| {
            let (nodes, edges) = crate::ingest::read_records(text.as_bytes()).expect("valid");
            (crate::ingest::index(&nodes, &edges).expect("fits"), nodes.len())
        })
    });
    report("studio", text.len(), nodes.1, heap, rise);
    drop((text, nodes));
    let text = contract_document(BODY);
    let ((derived, heap), rise) =
        resident_rise(|| measure(|| crate::contract::derive(text.as_bytes()).expect("derives")));
    report("contract", text.len(), derived.0.nodes.len(), heap, rise);
}

#[test]
fn a_small_generated_contract_document_derives_every_record_and_its_tags() {
    let text = contract_document(4_000);
    let records = text.matches(r#""collection":"task","deleted""#).count();
    let (derived, _) = crate::contract::derive(text.as_bytes()).expect("derives");
    // Every record is a node, and the 16 tag values each derive one tag node.
    assert_eq!(derived.nodes.len(), records + 16.min(records));
    assert!(records > 16, "the document is long enough to use every tag");
}

fn report(source: &str, body: usize, nodes: usize, heap: usize, rise: Option<u64>) {
    let rise = rise.map_or_else(|| "not measured".to_owned(), |bytes| bytes.to_string());
    println!("| {source} | {body} | {nodes} | {heap} | {rise} |");
}

/// What the resident size rose by while `run` ran: VmHWM after it, less VmRSS at a peak reset
/// before it. `None` where `/proc/self/clear_refs` refuses the reset, as outside Linux.
///
/// Caveat: pages the allocator kept from an earlier free are reused without raising the
/// resident size, so the rise under-reports by up to what was freed and kept before `run`.
/// The heap peak beside it does not, which is why both are printed.
fn resident_rise<T>(run: impl FnOnce() -> T) -> (T, Option<u64>) {
    let reset = std::fs::write("/proc/self/clear_refs", "5").is_ok();
    let base = status_kib("VmRSS");
    let value = run();
    let rise = match (reset, base, status_kib("VmHWM")) {
        (true, Some(base), Some(high)) => Some(high.saturating_sub(base) * 1024),
        _ => None,
    };
    (value, rise)
}

/// A `kB` field of `/proc/self/status`. A second copy of graph-cli's `cap_probe::rss`
/// parse: graph-wasm cannot depend on graph-cli.
fn status_kib(field: &str) -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    status.lines().find_map(|line| {
        let value = line.strip_prefix(field)?.strip_prefix(':')?.trim();
        value.strip_suffix("kB")?.trim().parse().ok()
    })
}

/// The seeded model at the node count whose ingest text is about `target` bytes, scaled
/// from the text length at 10k nodes.
fn studio_document(target: usize) -> String {
    let probe = 10_000_u32;
    let per_node = seeded_text(probe).len() / probe as usize;
    seeded_text(u32::try_from(target / per_node).expect("fits u32"))
}

fn seeded_text(n: u32) -> String {
    let (nodes, edges) = graph_core::seeded_model(1, n, graph_core::REFERENCE_DEGREE);
    crate::seed_ingest::document(&nodes, &edges).expect("finite")
}

/// Records are appended until the text reaches `target` bytes. Record `i` has parent
/// `(i - 1) / 2` and blocks one earlier record, so every link resolves.
fn contract_document(target: usize) -> String {
    let mut out = String::from(CONTRACT_HEAD);
    let mut i = 0_usize;
    while out.len() < target {
        if i > 0 {
            out.push(',');
        }
        record(&mut out, i);
        i += 1;
    }
    out.push_str("]}");
    out
}

fn record(out: &mut String, i: usize) {
    let (tag, effort) = (i % 16, i % 7 + 1);
    write!(
        out,
        r#"{{"id":"r{i}","collection":"task","deleted":false,"updatedAt":{i},"values":{{"name":"Task {i}","labels":["g{tag}"],"effort":{effort}"#
    )
    .expect("a String takes every write");
    if i > 0 {
        let (up, blocks) = ((i - 1) / 2, i.wrapping_mul(7919) % i);
        write!(out, r#","up":"r{up}","blocks":["r{blocks}"]"#)
            .expect("a String takes every write");
    }
    out.push_str("}}");
}
