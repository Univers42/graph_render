//! The registration half of the gate: every row in [`POSTS`] declares its metadata, and
//! [`find`] answers with the row and the run that row registers.

use super::*;

/// Every registered POST capability declares its metadata, and every one in the matrix is
/// registered. The negative control is the last two rows: a capability that composes but is
/// absent from the registry would pass `post_composability` and never appear in the ledger.
#[test]
fn every_registered_capability_declares_its_metadata() {
    let mut ids: Vec<&str> = POSTS.iter().map(|cap| cap.id).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), POSTS.len(), "unique ids");
    for cap in POSTS.iter() {
        let meta = cap.meta;
        assert!(cap.id.starts_with("post."), "{}", cap.id);
        assert!(meta.tier >= 1, "{}", cap.id);
        assert!(meta.scale_ceiling > 0, "{}", cap.id);
        for text in [
            meta.oracle,
            meta.complexity,
            meta.degradation,
            meta.ponytail,
        ] {
            assert!(!text.trim().is_empty(), "{}", cap.id);
        }
        assert!(
            meta.ponytail.contains("Ponytail") || meta.ponytail.contains("No Ponytail"),
            "{}: a capability states its markers or says none is owed",
            cap.id
        );
    }
    assert!(find("post.bundle.none").is_none());
    assert!(find("post.bundle.FDEB").is_none(), "ids are exact");
    assert!(find("layout.grid").is_none(), "post ids, not layout ids");
}

/// The registered run and the id in one row: [`find`] answers for the capability that is
/// actually in [`POSTS`], and the run it hands back is the one the matrix calls. Without the
/// first half a `find` that answered for every id would pass; without the second, a row whose
/// `run` pointed at another capability's entry point would too.
#[test]
fn find_answers_with_the_row_and_the_run_that_row_registers() {
    for cap in &POSTS {
        let found = find(cap.id).expect("registered");
        assert_eq!(found.id, cap.id);
        let topology = topology();
        let geometry = (LAYOUTS[0].run)(&topology).expect("the grid lays out the test graph");
        assert_eq!(
            (found.run)(&topology, &geometry).map(|b| b.geometry),
            (cap.run)(&topology, &geometry).map(|b| b.geometry),
            "{}",
            cap.id
        );
    }
}
