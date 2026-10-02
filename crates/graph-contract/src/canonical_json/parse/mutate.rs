//! Seeded edits for the reader's differential corpus. Byte-level (a flipped byte, a
//! truncation) and structural (a member duplicated or renamed, a value's type swapped, the
//! text wrapped in more nesting), drawn from one `mulberry32` so the corpus is the same
//! bytes on every machine and every run (D3). No dependency: the generator is here.
//!
//! The ingest reader has its own, larger edit set — it needs id and endpoint edits that
//! mean nothing to a generic JSON reader. Two copies of a twenty-line generator, rather
//! than a test helper in the public API of a contract crate, is the cheaper trade.

/// One seed, and the draws taken from it.
pub(super) struct Rng(u32);

impl Rng {
    pub(super) fn new(seed: u32) -> Self {
        Rng(seed)
    }

    /// The canonical `mulberry32` step: `f64` in `[0, 1)`, identical in native and wasm32.
    pub(super) fn unit(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x6D2B_79F5);
        let mut t = self.0;
        t = (t ^ (t >> 15)).wrapping_mul(1 | t);
        t = t.wrapping_add((t ^ (t >> 7)).wrapping_mul(61 | t)) ^ t;
        let n = f64::from(t ^ (t >> 14));
        n / 4_294_967_296.0
    }

    /// An index in `0..len`, or `0` for an empty range.
    pub(super) fn below(&mut self, len: usize) -> usize {
        if len == 0 {
            return 0;
        }
        ((self.unit() * len as f64) as usize).min(len - 1)
    }
}

/// One edit, named so a failure reads as what was done to the text.
#[derive(Debug, Clone, Copy)]
pub(super) enum Edit {
    /// One character replaced by one that makes JSON mean something else.
    Flip,
    /// The text cut short at a random offset.
    Truncate,
    /// A `"key":value,` pair written twice, so the reader must refuse the repeat.
    DuplicateMember,
    /// A key's spelling changed, so the member reads as a different one.
    RenameMember,
    /// A member's value replaced by a literal of another JSON type.
    SwapType,
    /// The whole value wrapped in brackets, so the text nests deeper than before.
    Nest,
    /// A bracket or a quote dropped, so the text stops being a value at all.
    DropByte,
}

pub(super) const ALL_EDITS: [Edit; 7] = [
    Edit::Flip,
    Edit::Truncate,
    Edit::DuplicateMember,
    Edit::RenameMember,
    Edit::SwapType,
    Edit::Nest,
    Edit::DropByte,
];

/// Characters that change what a text means: structural punctuation and literal letters.
const POOL: [char; 14] = [
    '{', '}', '[', ']', '"', ',', ':', '\\', 'n', 't', 'f', '0', '9', ' ',
];

/// Literals of one JSON type standing where another is expected, so the reader's
/// `not the start of a value` and number paths are reached.
const SWAPS: [&str; 6] = ["null", "true", "0", "1.5", r#""""#, r#""x""#];

/// The text one edit produced, or `None` when there is nowhere to apply it.
pub(super) fn apply(text: &str, edit: Edit, rng: &mut Rng) -> Option<String> {
    if text.is_empty() {
        return None;
    }
    match edit {
        Edit::Flip => Some(flip(text, rng)),
        Edit::Truncate => Some(text[..boundary(text, rng.below(text.len()))].to_owned()),
        Edit::DuplicateMember => duplicate_member(text, rng),
        Edit::RenameMember => rename_member(text, rng),
        Edit::SwapType => swap_type(text, rng),
        Edit::Nest => {
            let depth = 1 + rng.below(3);
            Some(format!("{}{text}{}", "[".repeat(depth), "]".repeat(depth)))
        }
        Edit::DropByte => {
            let at = boundary(text, rng.below(text.len()));
            let mut out = text.to_owned();
            out.remove(at);
            Some(out)
        }
    }
}

fn flip(text: &str, rng: &mut Rng) -> String {
    let at = boundary(text, rng.below(text.len()));
    let replacement = POOL[rng.below(POOL.len())];
    let after = at + text[at..].chars().next().map_or(1, char::len_utf8);
    let mut out = String::with_capacity(text.len() + 4);
    out.push_str(&text[..at]);
    out.push(replacement);
    out.push_str(&text[after..]);
    out
}

/// The first char boundary at or after `at`: a byte offset drawn over the whole text may
/// land inside a multi-byte character, and an edit has to stay a `String`.
fn boundary(text: &str, at: usize) -> usize {
    let mut at = at.min(text.len());
    while !text.is_char_boundary(at) {
        at += 1;
    }
    at
}

/// Byte offsets of every occurrence of `needle`.
fn spots(text: &str, needle: &str) -> Vec<usize> {
    let mut at = 0;
    let mut found = Vec::new();
    while let Some(off) = text[at..].find(needle) {
        let start = at + off;
        found.push(start);
        at = start + needle.len();
    }
    found
}

/// The `"key":` pairs of the text: where each key starts, and where its value ends.
fn member_spans(text: &str) -> Vec<(usize, usize)> {
    spots(text, "\":")
        .into_iter()
        .filter_map(|colon| {
            let key_start = text[..colon].rfind('"')?;
            let end = value_end(text, colon + 2)?;
            Some((key_start, end))
        })
        .collect()
}

/// Where the value after `from` ends: past a string, a literal, or a closed bracket.
fn value_end(text: &str, from: usize) -> Option<usize> {
    let rest = text.get(from..)?;
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for (i, c) in rest.char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
            }
            continue;
        }
        match c {
            '"' => in_string = true,
            '{' | '[' => depth += 1,
            '}' | ']' => {
                if depth == 0 {
                    return Some(from + i);
                }
                depth -= 1;
                if depth == 0 {
                    return Some(from + i + 1);
                }
            }
            ',' if depth == 0 => return Some(from + i),
            _ => {}
        }
    }
    None
}

fn duplicate_member(text: &str, rng: &mut Rng) -> Option<String> {
    let spans = member_spans(text);
    let (key, end) = *spans.get(rng.below(spans.len()))?;
    let pair = text.get(key..end)?;
    let mut out = String::with_capacity(text.len() + pair.len());
    out.push_str(&text[..end]);
    out.push(',');
    out.push_str(pair);
    out.push_str(&text[end..]);
    Some(out)
}

fn rename_member(text: &str, rng: &mut Rng) -> Option<String> {
    let spans = member_spans(text);
    let (key, _) = *spans.get(rng.below(spans.len()))?;
    let mut out = String::with_capacity(text.len() + 1);
    out.push_str(&text[..key + 1]);
    out.push('_');
    out.push_str(&text[key + 1..]);
    Some(out)
}

fn swap_type(text: &str, rng: &mut Rng) -> Option<String> {
    let spans = member_spans(text);
    let (key, end) = *spans.get(rng.below(spans.len()))?;
    let swapped = SWAPS[rng.below(SWAPS.len())];
    let mut out = String::with_capacity(text.len() + swapped.len());
    out.push_str(&text[..key + 3]);
    out.push_str(swapped);
    out.push_str(&text[end..]);
    Some(out)
}
