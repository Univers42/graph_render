//! Identity helpers (`src/core/model/ids.ts`) and the palette hash (`src/core/math.ts:11`).
//!
//! Node ids mirror the BaaS coordinate scheme, so a record keeps its id — and therefore
//! its layout position — across rebuilds. Edge ids are content-addressed, so A–B and B–A
//! collapse when undirected.

use crate::edgekind::EdgeKind;

/// The two id prefixes `make_record_node_id` never produces (`ids.ts:42-43`).
const NOTE_PREFIX: &str = "note:";
const TAG_PREFIX: &str = "tag:";

/// `source:databaseId:recordId` (`ids.ts:11`).
pub fn make_record_node_id(source: &str, database_id: &str, record_id: &str) -> String {
    format!("{source}:{database_id}:{record_id}")
}

/// `note:<noteId>` (`ids.ts:16`).
pub fn make_note_node_id(note_id: &str) -> String {
    format!("{NOTE_PREFIX}{note_id}")
}

/// `tag:<tagValue>` (`ids.ts:21`).
pub fn make_tag_node_id(tag_value: &str) -> String {
    format!("{TAG_PREFIX}{tag_value}")
}

/// The coordinates of a record node id, as [`parse_node_id`] reads them back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordRef<'a> {
    /// Backend that owns the record.
    pub source: &'a str,
    /// Database or collection.
    pub database_id: &'a str,
    /// Record key; may itself contain `:`.
    pub record_id: &'a str,
}

/// Splits a record node id back into its coordinates (`ids.ts:68-72`); `None` for a
/// `note:` or `tag:` id, which are not record ids.
///
/// Ponytail: H5, carried forward from the oracle's own PONYTAIL (`ids.ts:61-66`). The
/// grammar cannot represent `:` inside `source` or `databaseId`: no parse can tell
/// which colon was the separator. So `make_record_node_id("my:db", "x", "1")` does not
/// round-trip — the parse returns a *shifted, wrong* `{ source: "my", databaseId:
/// "db", recordId: "x:1" }`, not `None`, and nothing downstream can detect it. It gets
/// worse as data sources broaden, because arbitrary sources contain `:`. Escape hatch:
/// a caller-side constraint (reject `:` in those two coordinates before building the
/// id); this function preserves the oracle's behaviour, which is to parse anyway.
pub fn parse_node_id(node_id: &str) -> Option<RecordRef<'_>> {
    if node_id.starts_with(NOTE_PREFIX) || node_id.starts_with(TAG_PREFIX) {
        return None;
    }
    let mut parts = node_id.splitn(3, ':');
    Some(RecordRef {
        source: parts.next().unwrap_or(""),
        database_id: parts.next().unwrap_or(""),
        record_id: parts.next().unwrap_or(""),
    })
}

/// Deterministic edge id (`ids.ts:79-89`). Directed edges keep their orientation;
/// undirected ones order their endpoints so A–B and B–A are one edge.
///
/// **H1, decided** (`docs/decisions/h1-byte-order.md`): the endpoints are ordered by
/// bytes (`str::cmp`), not by `localeCompare` as in the oracle. The two disagree on
/// mixed case (`"a"`/`"A"`), on the `Z`/`a` boundary and on `note:1`/`NOTE:1`; for those
/// pairs this id intentionally differs from the oracle's by endpoint order.
///
/// Ponytail: orientation-blind, for oracle parity — `child_first` is not an argument, so
/// an A→B `child_of` and an A→B `parent_of` with the same label get one id although they
/// name opposite parents. Direction: two distinct hierarchy facts collapse to one id and
/// the later is dropped as a duplicate by `index_model`. Escape hatch: the host gives
/// the two edges distinct labels (or ids of its own).
pub fn make_edge_id(edge: &EdgeIdParts<'_>) -> String {
    let EdgeIdParts {
        source,
        target,
        kind,
        label,
        directed,
    } = *edge;
    let kind = kind.as_str();
    if directed {
        return format!("{source}->{target}:{kind}:{label}");
    }
    let (low, high) = if source <= target {
        (source, target)
    } else {
        (target, source)
    };
    format!("{low}--{high}:{kind}:{label}")
}

/// The five arguments of the oracle's `makeEdgeId`, in one value (house limit: four
/// parameters).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EdgeIdParts<'a> {
    /// The edge's source node id.
    pub source: &'a str,
    /// The edge's target node id.
    pub target: &'a str,
    /// The edge's kind.
    pub kind: EdgeKind,
    /// The edge's label, possibly empty.
    pub label: &'a str,
    /// Whether `source → target` is kept as given.
    pub directed: bool,
}

/// The oracle's `hashString` (`math.ts:11-16`), bit for bit: `h = imul(31, h) + c` over
/// UTF-16 code units, then `Math.abs`.
///
/// Two JS details are reproduced, not tidied. `codePointAt(i)` walks code *units*: at
/// the high half of a surrogate pair it yields the whole code point, and the loop then
/// visits the low half too. And the sum is not wrapped — only the next `imul` wraps it —
/// so the final value can exceed `i32::MAX` and is returned as a `u32`.
///
/// Ponytail: `i32::MIN`. When the accumulated value is exactly −2³¹, `Math.abs` gives
/// 2³¹ as a float, where Rust's `i32::abs` would overflow. This returns 2³¹, the
/// oracle's observable value ("xfjfxtf" is one such input); a caller narrowing the
/// result to `i32` gets it wrong silently. Pinned by a test.
pub fn hash_string(value: &str) -> u32 {
    let mut hash: i64 = 0;
    for point in code_points_at(value) {
        hash = i64::from((hash as i32).wrapping_mul(31)) + i64::from(point);
    }
    hash.unsigned_abs() as u32
}

/// `value.codePointAt(i)` for every code unit index `i`. A `&str` holds no lone
/// surrogate, so a character outside the BMP is always a whole pair: its code point at
/// the high half, then the low half by itself.
fn code_points_at(value: &str) -> impl Iterator<Item = u32> + '_ {
    value.chars().flat_map(|c| {
        let mut units = [0u16; 2];
        let low = match *c.encode_utf16(&mut units) {
            [_, low] => Some(u32::from(low)),
            _ => None,
        };
        core::iter::once(u32::from(c)).chain(low)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_ids_join_their_coordinates() {
        assert_eq!(make_record_node_id("pg", "db", "7"), "pg:db:7");
        assert_eq!(make_note_node_id("n1"), "note:n1");
        assert_eq!(make_tag_node_id("vintage"), "tag:vintage");
    }

    #[test]
    fn parse_reads_record_ids_back_and_refuses_note_and_tag_ids() {
        let parsed = parse_node_id("pg:db:7").expect("a record id");
        assert_eq!(
            (parsed.source, parsed.database_id, parsed.record_id),
            ("pg", "db", "7")
        );
        assert_eq!(parse_node_id("note:n1"), None);
        assert_eq!(parse_node_id("tag:vintage"), None);
        assert!(
            parse_node_id("tag").is_some(),
            "only the prefix with its colon"
        );
    }

    #[test]
    fn parse_rejoins_composite_keys_and_defaults_missing_segments() {
        let composite = parse_node_id("pg:db:a:b:c").expect("record");
        assert_eq!(composite.record_id, "a:b:c");
        let short = parse_node_id("pg").expect("record");
        assert_eq!((short.source, short.database_id), ("pg", ""));
        assert_eq!(short.record_id, "");
        let empty = parse_node_id("").expect("record");
        assert_eq!((empty.source, empty.database_id), ("", ""));
    }

    #[test]
    fn parse_shifts_a_colon_in_source_instead_of_refusing_h5() {
        let id = make_record_node_id("my:db", "x", "1");
        let parsed = parse_node_id(&id).expect("parses anyway");
        assert_eq!((parsed.source, parsed.database_id), ("my", "db"));
        assert_eq!(parsed.record_id, "x:1");
    }

    #[test]
    fn edge_ids_keep_direction_or_order_endpoints_by_bytes() {
        let id = |(source, target): (&str, &str), kind, label, directed| {
            make_edge_id(&EdgeIdParts {
                source,
                target,
                kind,
                label,
                directed,
            })
        };
        let kind = EdgeKind::NoteLink;
        assert_eq!(id(("b", "a"), kind, "l", true), "b->a:note_link:l");
        assert_eq!(id(("b", "a"), kind, "l", false), "a--b:note_link:l");
        assert_eq!(id(("a", "b"), kind, "", false), "a--b:note_link:");
        // H1: byte order puts "A" (0x41) before "a" (0x61); localeCompare does not.
        assert_eq!(id(("a", "A"), EdgeKind::Tag, "", false), "A--a:tag:");
        let prefixed = id(("note:1", "NOTE:1"), EdgeKind::Relation, "x", false);
        assert_eq!(prefixed, "NOTE:1--note:1:relation:x");
        assert_eq!(
            id(("x", "x"), EdgeKind::Relation, "", false),
            "x--x:relation:"
        );
    }

    #[test]
    fn hash_string_matches_the_oracle() {
        // Values printed by `hashString` from src/core/math.ts under node:22-slim.
        let cases = [
            ("", 0),
            ("a", 97),
            ("ab", 3105),
            ("db-3", 3_074_724),
            ("bench:db-0:12", 495_553_968),
            ("\u{1F680}", 4_044_800),
            ("xfjfxtf", 2_147_483_648),
            ("xfjfxtf\0", 2_147_483_648),
            // Past −2³¹ the sign of the sum shows: a negated hash would give 2147483745.
            ("xfjfxtfa", 2_147_483_551),
        ];
        for (text, want) in cases {
            assert_eq!(hash_string(text), want, "{text:?}");
        }
    }
}
