//! The caps table (Verdict condition 2): exactly one row per id the service runs, none for an
//! id it does not, and no cap past the ceiling the ledger records for that id.

mod common;

use graph_server::caps::{Cap, Caps};
use graph_server::motor;
use std::collections::BTreeSet;

/// The ledger's ceiling of `id`: a layout's counts nodes, a POST pass's counts edges.
/// `post.route.grid` has none in graph-core (graph-cli holds its 5000-node ceiling).
fn ceiling(id: &str) -> (Option<u64>, Option<u64>) {
    if let Some(layout) = graph_core::registry::find(id) {
        return (Some(layout.meta.scale_ceiling), None);
    }
    if let Some(post) = graph_core::post::find(id) {
        return (None, Some(post.meta.scale_ceiling));
    }
    let style = graph_core::post::styles::find(id);
    (None, style.map(|style| style.meta.scale_ceiling))
}

#[test]
fn every_service_id_has_exactly_one_row() {
    let caps = Caps::committed().expect("the committed table parses");
    let have: BTreeSet<&str> = caps.ids().collect();
    let wanted: Vec<&str> = motor::layout_ids().chain(motor::post_ids()).collect();
    let missing: Vec<&&str> = wanted.iter().filter(|id| !have.contains(*id)).collect();
    assert!(missing.is_empty(), "ids with no row: {missing:?}");
    let wanted: BTreeSet<&str> = wanted.into_iter().collect();
    let extra: Vec<&&str> = have.difference(&wanted).collect();
    assert!(
        extra.is_empty(),
        "rows for ids the service does not run: {extra:?}"
    );
}

#[test]
fn no_cap_is_past_its_ledger_ceiling() {
    let caps = Caps::committed().expect("the committed table parses");
    for id in caps.ids() {
        let cap = caps.get(id).expect("a listed id has a row");
        let (nodes, edges) = ceiling(id);
        assert!(
            nodes.is_none_or(|ceiling| cap.nodes <= ceiling),
            "{id}: {cap:?} > {nodes:?} nodes"
        );
        assert!(
            edges.is_none_or(|ceiling| cap.edges <= ceiling),
            "{id}: {cap:?} > {edges:?} edges"
        );
    }
}

#[test]
fn a_second_row_for_one_id_is_refused_by_line() {
    let table = "id\tcap_n\tcap_m\nforce\t1\t1\nforce\t2\t2\n";
    let refused = Caps::parse(table).expect_err("a duplicate is refused");
    assert!(refused.contains("line 3"), "{refused}");
}

#[test]
fn a_malformed_row_is_refused_by_line() {
    for (table, line) in [
        ("id\tcap_n\tcap_m\nforce\t1\n", "line 2"),
        ("id\tcap_n\tcap_m\nforce\t1\t-1\n", "line 2"),
        ("id cap_n cap_m\n", "first line"),
    ] {
        let refused = Caps::parse(table).expect_err("a malformed table is refused");
        assert!(refused.contains(line), "{table:?}: {refused}");
    }
}

/// The host every POST pass rides on: its cap is at least every pass's.
const HOST: &str = "layout.grid";

/// The query that runs `id`, and the ids whose caps apply to it, in the order they are checked.
fn ask(id: &str) -> (String, Vec<&str>) {
    if motor::post_ids().any(|post| post == id) {
        (format!("layout={HOST}&post={id}"), vec![HOST, id])
    } else {
        (format!("layout={id}"), vec![id])
    }
}

/// The id a graph of `size` is refused for: the first of `checked` whose cap it is past.
fn refused_for<'a>(caps: &Caps, checked: &[&'a str], size: (u64, u64)) -> Option<&'a str> {
    checked.iter().copied().find(|id| {
        let cap = caps.get(id).expect("a listed id has a row");
        size.0 > cap.nodes || size.1 > cap.edges
    })
}

/// Two graphs one past the cap of `id`: one node too many, and one edge too many on the fewest
/// nodes that hold that many distinct pairs in `common::doc`'s pattern.
fn one_past(cap: Cap) -> [(u64, u64); 2] {
    let edges = cap.edges + 1;
    let nodes = (2 * edges).isqrt() + 3;
    assert!(nodes <= cap.nodes, "{cap:?}: no graph is one edge past");
    [(cap.nodes + 1, 0), (nodes.max(16), edges)]
}

/// Every cap admits a graph at the cap and refuses one past it, naming the id that refused.
#[test]
fn every_cap_admits_its_bound_and_refuses_one_past() {
    let caps = Caps::committed().expect("the committed table parses");
    for id in caps.ids() {
        let (_, checked) = ask(id);
        let host = checked[0];
        let cap = caps.get(id).expect("a listed id has a row");
        let at = (cap.nodes, cap.edges);
        let admitted = caps.admit(host, checked.get(1).copied(), narrow(at));
        assert_eq!(admitted, Ok(()), "{id} at its cap");
        for size in one_past(cap) {
            let refusal = caps.admit(host, checked.get(1).copied(), narrow(size));
            let refusal = refusal.expect_err("one past the cap");
            let named = refused_for(&caps, &checked, size).expect("past a cap");
            assert_eq!(refusal.status, 413, "{id} at {size:?}");
            assert!(
                refusal.message.starts_with(named),
                "{id}: {}",
                refusal.message
            );
        }
    }
}

fn narrow((nodes, edges): (u64, u64)) -> (u32, u32) {
    let narrow = |count| u32::try_from(count).expect("a cap under 2^32");
    (narrow(nodes), narrow(edges))
}

/// What is sent one past a cap: the document when it fits the body limit, else only the length
/// it declares, which is refused unread.
enum Past {
    Doc(String),
    Declared(usize),
}

fn past(size: (u64, u64), max_body: usize) -> Past {
    let floor = common::doc_floor(size);
    if floor > max_body {
        return Past::Declared(floor);
    }
    let (n, m) = narrow(size);
    let doc = common::doc(n as usize, m as usize);
    if doc.len() > max_body {
        Past::Declared(doc.len())
    } else {
        Past::Doc(doc)
    }
}

/// Row `svc-caps`: through the router, one past the cap of every id is a 413 in under 1 s, so no
/// run starts. A document that fits the default 64 MiB body is refused by the work cap, naming the
/// id; a larger one is refused by the body cap before any ingest.
#[cfg_attr(
    debug_assertions,
    ignore = "row svc-caps runs it under --release: the 1 s bound is for an optimised build"
)]
#[tokio::test]
async fn one_past_every_cap_is_413_within_a_second() {
    let server = common::server(&[]);
    let caps = Caps::committed().expect("the committed table parses");
    let mut by_work_cap = Vec::new();
    for id in caps.ids() {
        let (query, checked) = ask(id);
        for size in one_past(caps.get(id).expect("a listed id has a row")) {
            let sent = past(size, server.app.limits.max_body);
            if matches!(sent, Past::Doc(_)) {
                by_work_cap.push(format!("{id} {size:?}"));
            }
            let by = match &sent {
                Past::Doc(_) => refused_for(&caps, &checked, size).expect("past a cap"),
                Past::Declared(_) => "the body is past GRAPH_MAX_BODY",
            };
            let started = std::time::Instant::now();
            let reply = match sent {
                Past::Doc(doc) => server.layout(&query, doc).await,
                Past::Declared(length) => server.declared(&query, length).await,
            };
            let took = started.elapsed();
            let named = (reply.status.as_u16(), reply.code(), reply.message());
            assert_eq!(
                (named.0, named.1.as_str()),
                (413, "IngestTooLarge"),
                "{id} {size:?}"
            );
            assert!(named.2.starts_with(by), "{id} at {size:?}: {}", named.2);
            assert!(took.as_millis() < 1000, "{id} at {size:?} took {took:?}");
        }
    }
    // The work-cap path must run at all, or the row only proves the body cap.
    assert!(!by_work_cap.is_empty(), "no graph reached a work cap");
    eprintln!("refused by a work cap: {}", by_work_cap.join(", "));
}

#[test]
fn the_document_floor_is_at_most_its_length() {
    for (n, m) in [(0, 0), (1, 0), (16, 40), (1234, 5000)] {
        let floor = common::doc_floor((n, m));
        let length = common::doc(n as usize, m as usize).len();
        assert!(floor <= length, "({n}, {m}): {floor} > {length}");
    }
}
