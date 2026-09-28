//! Edge kinds, and the classifier from a wire `type` string (`src/core/model/edgeKind.ts`).

/// Relationship flavour. The discriminant is the column byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum EdgeKind {
    /// Plain structural edge; also where every unknown wire type lands.
    Relation = 0,
    /// Record → tag hub.
    Tag = 1,
    /// Note annotating a record.
    NoteOf = 2,
    /// Note → note link.
    NoteLink = 3,
    /// Parent and child. Which end is the parent is the edge's `child_first` flag
    /// ([`child_first_from_type`]); `Topology::parent`/`child` read it.
    Hierarchy = 4,
}

impl EdgeKind {
    /// Every kind, in discriminant order.
    pub const ALL: [Self; 5] = [
        Self::Relation,
        Self::Tag,
        Self::NoteOf,
        Self::NoteLink,
        Self::Hierarchy,
    ];

    /// The oracle's string for this kind.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Relation => "relation",
            Self::Tag => "tag",
            Self::NoteOf => "note_of",
            Self::NoteLink => "note_link",
            Self::Hierarchy => "hierarchy",
        }
    }

    /// The kind whose [`as_str`](Self::as_str) is `name`, if any.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.as_str() == name)
    }
}

/// Classifies a wire edge `type` (`edgeKind.ts:41-79`), branch for branch.
///
/// Ponytail: a heuristic, not a parser, and branch order is load-bearing. Three of the
/// five tests match a *substring anywhere*, so a type containing two markers is decided
/// by order alone: `"note_link_hierarchy"` is `Hierarchy`, not `NoteLink`, because the
/// hierarchy test runs first. And **unknown types degrade silently to `Relation`** — a
/// renamed wire type such as `"child-of"` renders as a plain edge with no error, no
/// warning, and nothing in the output to detect it (the dangerous direction: silent
/// misclassification). Escape hatch: callers that need to know must compare the wire
/// type against the literals themselves; this function preserves the host's behaviour
/// exactly, so tightening it is a behaviour change, not a fix.
pub fn edge_kind_from_type(wire_type: Option<&str>) -> EdgeKind {
    let Some(wire_type) = wire_type.filter(|t| !t.is_empty()) else {
        return EdgeKind::Relation;
    };
    let lowered = wire_type.to_lowercase();
    let is = |literal: &str| lowered == literal;
    if is("parent") || is("parent_of") || is("child_of") || lowered.contains("hierarchy") {
        return EdgeKind::Hierarchy;
    }
    if lowered.contains("note_link") || is("links_to") {
        return EdgeKind::NoteLink;
    }
    if lowered.contains("note_of") || is("annotates") {
        return EdgeKind::NoteOf;
    }
    if is("tagged") || is("tag") {
        return EdgeKind::Tag;
    }
    EdgeKind::Relation
}

/// Whether a wire `type` names its **child first** — source the child, target the
/// parent — which is `child_of` and nothing else: its lowercased text equal to
/// `"child_of"`, exactly (user decision D-Q1, option c). `parent`, `parent_of` and every
/// `*hierarchy*` type keep the source as the parent, and a type that merely contains
/// `child_of` is not flipped. Kept apart from [`edge_kind_from_type`], whose result is
/// the oracle's and stays so.
pub fn child_first_from_type(wire_type: Option<&str>) -> bool {
    wire_type.is_some_and(|t| t.to_lowercase() == "child_of")
}

#[cfg(test)]
mod tests {
    use super::EdgeKind::*;
    use super::*;

    #[test]
    fn names_round_trip_and_unknown_names_are_none() {
        for kind in EdgeKind::ALL {
            assert_eq!(EdgeKind::from_name(kind.as_str()), Some(kind));
        }
        assert_eq!(EdgeKind::from_name("Relation"), None);
    }

    #[test]
    fn classifier_follows_the_oracle_branch_by_branch() {
        let cases = [
            (None, Relation),
            (Some(""), Relation),
            (Some("PARENT"), Hierarchy),
            (Some("parent_of"), Hierarchy),
            (Some("child_of"), Hierarchy),
            (Some("x_Hierarchy_y"), Hierarchy),
            (Some("my_note_link"), NoteLink),
            (Some("Links_To"), NoteLink),
            (Some("note_of_x"), NoteOf),
            (Some("annotates"), NoteOf),
            (Some("TAGGED"), Tag),
            (Some("tag"), Tag),
            (Some("parent_tag"), Relation),
            (Some("vintage"), Relation),
            (Some("child-of"), Relation),
        ];
        for (wire, want) in cases {
            assert_eq!(edge_kind_from_type(wire), want, "{wire:?}");
        }
    }

    #[test]
    fn only_an_exact_child_of_puts_the_child_first() {
        for wire in ["child_of", "CHILD_OF", "Child_Of"] {
            assert!(child_first_from_type(Some(wire)), "{wire}");
            assert_eq!(edge_kind_from_type(Some(wire)), Hierarchy, "{wire}");
        }
        let parent_first = [
            None,
            Some(""),
            Some("parent"),
            Some("parent_of"),
            Some("x_hierarchy_y"),
            Some("child_of_hierarchy"),
            Some("child-of"),
        ];
        for wire in parent_first {
            assert!(!child_first_from_type(wire), "{wire:?}");
        }
        assert_eq!(edge_kind_from_type(Some("child-of")), Relation);
        assert_eq!(edge_kind_from_type(Some("child_of_hierarchy")), Hierarchy);
    }

    #[test]
    fn branch_order_decides_a_type_with_two_markers() {
        assert_eq!(edge_kind_from_type(Some("note_link_hierarchy")), Hierarchy);
        assert_eq!(edge_kind_from_type(Some("note_link_note_of")), NoteLink);
    }
}
