use super::*;

/// Two real nodes, `0 -> 1`, whose `roots` lists whatever the test says, a stale index
/// included.
struct Listed(&'static [u32]);

impl Roots for Listed {
    fn node_count(&self) -> u32 {
        2
    }

    fn roots(&self) -> &[u32] {
        self.0
    }

    fn virtual_root(&self) -> Option<u32> {
        (self.0.len() >= 2).then_some(2)
    }

    fn children(&self, v: u32) -> &[u32] {
        if v == 0 { &[1] } else { &[] }
    }
}

/// M29 (`docs/reviews/review-core-post.md`): a `roots()` naming the virtual root `n`
/// is refused with the same message `depth_from` gives, not an opaque slice index.
#[test]
#[should_panic(expected = "root 2 of 2")]
fn a_detected_root_past_the_last_node_is_refused_with_its_index() {
    bfs_depth(&Listed(&[0, 2]));
}

/// M30: a caller tells an unreached node from a real level without knowing the sentinel.
#[test]
fn an_unreached_node_reads_as_unreached_and_a_reached_one_does_not() {
    let partial = depth_from(&Listed(&[0]), &[1]);
    assert!(partial.is_unreached(0));
    assert!(!partial.is_unreached(1));
    let whole = bfs_depth(&Listed(&[0]));
    assert!(!whole.is_unreached(0) && !whole.is_unreached(1));
}
