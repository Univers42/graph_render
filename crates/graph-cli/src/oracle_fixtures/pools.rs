//! The generator's value pools: adversarial on purpose (see `generate.rs`).

/// Ids where byte order and `localeCompare` disagree, and ids that parse oddly.
pub const ID_POOL: [&str; 22] = [
    "a",
    "A",
    "b",
    "B",
    "Z",
    "z",
    "note:1",
    "NOTE:1",
    "tag:x",
    "\u{e9}",
    "e",
    "_x",
    "ax",
    "10",
    "9",
    "a-b",
    "ab",
    "",
    "pg:db:1",
    "pg:db:1:2",
    "x",
    "\u{1F680}",
];
/// Labels: empty, accented, CJK, JSON escapes, control characters, an emoji sequence.
pub const TEXT_POOL: [&str; 9] = [
    "",
    "Label",
    "\u{e9}t\u{e9}",
    "\u{65e5}\u{672c}",
    "a\"b\\c",
    "line\nbreak",
    "tab\there",
    "\u{1F5FA}\u{FE0F}",
    "\u{7f}\u{1}",
];
/// Wire `type` strings for `edgeKindFromType`: every branch, near misses, and a Kelvin
/// sign (U+212A) that `toLowerCase` folds to `k`.
pub const TYPE_POOL: [&str; 24] = [
    "parent",
    "PARENT",
    "parent_of",
    "child_of",
    "Child_Of",
    "x_hierarchy_y",
    "HIERARCHY",
    "note_link",
    "my_note_link",
    "links_to",
    "LIN\u{212A}S_TO",
    "note_of",
    "annotates",
    "tagged",
    "TAG",
    "tag",
    "parent_tag",
    "vintage",
    "stage",
    "child-of",
    "note_link_note_of",
    "note_link_hierarchy",
    "",
    "relation",
];
/// Weights and strengths, including `-0`, a subnormal-adjacent value and `NaN`.
pub const FLOATS: [f64; 8] = [0.5, 0.0, -0.0, 1.0, 0.2, 1e-300, 1.0 / 3.0, f64::NAN];
/// Versions, including 2^53.
pub const VERSIONS: [f64; 6] = [0.0, 1.0, 2.0, -1.0, 9_007_199_254_740_992.0, 0.5];
