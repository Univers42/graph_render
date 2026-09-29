//! The registration each style answers to: four unique capability ids, each in the
//! `post` stage, each declaring the kind and degree its implementation emits. Split from
//! the parent, which holds the case builders.

use super::*;

/// Every style capability is registered once, under the id its metadata declares,
/// in the `post` stage, with the kind and degree it actually emits.
#[test]
fn every_registered_style_agrees_with_its_implementation() {
    let ids: Vec<&str> = STYLES.iter().map(|c| c.id).collect();
    assert_eq!(
        ids,
        [
            "post.style.straight",
            "post.style.orthogonal",
            "post.style.bezier",
            "post.style.quadratic",
        ]
    );
    for capability in &STYLES {
        let style = Style::from_id(capability.id).expect("a style id");
        assert_eq!(capability.meta.stage, "post");
        assert_eq!(capability.meta.edges, style.kind(), "{}", capability.id);
        assert!(capability.meta.scale_ceiling > 0, "{}", capability.id);
    }
}
