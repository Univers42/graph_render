use super::*;

/// The map evicts the least recently used entry once it is full.
#[tokio::test]
async fn last_seen_evicts_least_recently_used_first() {
    let mut map = LastSeen::new(2);
    map.raise("oldest", 1, 1);
    map.raise("middle", 1, 1);
    // Touch `oldest` so `middle` becomes the least recently used.
    map.raise("oldest", 2, 2);
    map.raise("newest", 1, 1);
    assert_eq!(map.len(), 2, "the map grew past its cap");
    assert!(map.get("middle").is_none(), "the middle entry survived");
    assert_eq!(
        map.get("oldest"),
        Some((2, 2)),
        "the touched entry was evicted"
    );
    assert!(map.get("newest").is_some(), "the newest entry was evicted");
}

/// Entries only rise: a lower `(epoch, seq)` is ignored, so a stale writer cannot walk back.
#[tokio::test]
async fn last_seen_entries_only_rise() {
    let mut map = LastSeen::new(8);
    map.raise("ws", 5, 9);
    map.raise("ws", 5, 3);
    assert_eq!(map.get("ws"), Some((5, 9)), "a lower seq lowered the entry");
    map.raise("ws", 6, 1);
    assert_eq!(map.get("ws"), Some((6, 1)), "a higher epoch did not rise");
}
