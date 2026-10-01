//! The reader is one rule over every record, not one arm per Graphviz engine — so the arms
//! that are not Graphviz read through it too, and a name nothing writes is still refused.
//! Split out of the parent to keep both files under the house line limit.

use super::super::record_of;
use super::{BACKED, find_row, hand, honest, recorded};

/// The igraph, spring, closed-form and hand-oracle arms read through the same one rule, so
/// the reader gap `unproven.rs` documented is gone for all of them and not only for the
/// Graphviz six.
#[test]
fn the_other_differential_arms_read_through_the_same_rule() {
    for (id, record) in [
        ("layout.force.spring", "oracle-spring"),
        ("layout.circular.hierarchy", "oracle-circular-hierarchy"),
        ("layout.random", "oracle-closed-form"),
        ("layout.force.fruchterman_reingold", "oracle-igraph"),
    ] {
        let mut evidence = honest();
        recorded(&mut evidence, record, id);
        assert_eq!(find_row(&evidence, id).oracle_diff, BACKED, "{id}");
    }
}

/// A name nothing writes is still refused, so the one rule is not a blank cheque: a record
/// under some other name backs nothing, and a row whose record is absent reads unbacked.
#[test]
fn a_name_nothing_writes_still_backs_nothing() {
    let mut evidence = honest();
    recorded(&mut evidence, "hand", "layout.random");
    assert!(
        find_row(&evidence, "layout.random")
            .oracle_diff
            .contains("no oracle-closed-form record"),
        "a record under another name backs nothing"
    );
    assert!(
        find_row(&evidence, "layout.force.spring")
            .oracle_diff
            .contains("no oracle-spring record"),
        "an absent record backs nothing"
    );
    // The fixture is a real record under a name no row names, so the two refusals above are
    // about the name and not about a map nothing could be read out of.
    assert_eq!(
        record_of(&evidence, "hand").expect("in the map")["functions"]["layout.random"]["cases"],
        hand(5)["cases"],
    );
}
