//! Seeded mutations of the ingest documents, for the differential test's second corpus.
//!
//! Byte-level (a flipped byte, a truncation) and structural (a member deleted, renamed or
//! duplicated, a value's type swapped, an `id` duplicated, an endpoint renamed) edits, all
//! drawn from one `mulberry32` so the corpus is the same bytes on every machine and every
//! run (D3). No dependency: the generator is thirty lines of it here, the same construction
//! as `graph_core::synthetic`'s, so a failing seed can be reproduced by index alone.

/// One seed, and how many draws it gets from the mutation budget.
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
        let scaled = (self.unit() * len as f64) as usize;
        scaled.min(len - 1)
    }
}

/// One edit, named so a differential failure reads as what was done to the bytes.
#[derive(Debug, Clone, Copy)]
pub(super) enum Edit {
    /// One byte replaced by another drawn from the JSON punctuation pool.
    Flip,
    /// The text cut short at a random offset.
    Truncate,
    /// A whole `"key":value,` pair removed.
    DeleteMember,
    /// A key's spelling changed, so the member reads as unknown.
    RenameMember,
    /// A `"key":value,` pair written twice, so the parser must refuse the repeat.
    DuplicateMember,
    /// A value replaced by a literal of a different JSON type.
    SwapType,
    /// An `id` given the value of another `id`, so the document must refuse the duplicate.
    DuplicateId,
    /// An edge endpoint pointed at an id no node carries.
    RenameEndpoint,
}

/// Every edit, in a fixed order, so `Edit` reads as an index into the kinds.
pub(super) const ALL_EDITS: [Edit; 8] = [
    Edit::Flip,
    Edit::Truncate,
    Edit::DeleteMember,
    Edit::RenameMember,
    Edit::DuplicateMember,
    Edit::SwapType,
    Edit::DuplicateId,
    Edit::RenameEndpoint,
];

/// The bytes one edit produced, or `None` when the document has nowhere to apply it (an
/// empty one, or a document with no `id` to duplicate).
pub(super) fn apply(text: &str, edit: Edit, rng: &mut Rng) -> Option<String> {
    let bytes = text.as_bytes();
    if bytes.is_empty() {
        return None;
    }
    match edit {
        Edit::Flip => Some(flip(text, rng)),
        Edit::Truncate => Some(text[..boundary(text, rng.below(text.len()))].to_owned()),
        Edit::DeleteMember => delete_member(text, rng),
        Edit::RenameMember => rename_member(text, rng),
        Edit::DuplicateMember => duplicate_member(text, rng),
        Edit::SwapType => swap_type(text, rng),
        Edit::DuplicateId => duplicate_id(text, rng),
        Edit::RenameEndpoint => rename_endpoint(text, rng),
    }
}

/// A byte swapped for one of the characters that make JSON mean something different:
/// structural punctuation, or the first letter of a literal.
const POOL: [char; 14] = [
    '{', '}', '[', ']', '"', ',', ':', '\\', 'n', 't', 'f', '0', '9', ' ',
];

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

/// The first char boundary at or after `at`: a byte offset drawn over the whole text
/// may land inside a multi-byte character, and a mutation has to stay a `String`.
fn boundary(text: &str, at: usize) -> usize {
    let mut at = at.min(text.len());
    while !text.is_char_boundary(at) {
        at += 1;
    }
    at
}

/// Byte offsets of every occurrence of `needle` in `text`.
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

/// Where the value after `from` ends: past a string, a literal, or a closed bracket.
/// `None` when the value runs off the end of the text, which is itself a useful mutation.
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

fn delete_member(text: &str, rng: &mut Rng) -> Option<String> {
    let spans = member_spans(text);
    let (key, end) = *spans.get(rng.below(spans.len()))?;
    let mut out = String::with_capacity(text.len());
    out.push_str(&text[..key]);
    out.push_str(text.get(end..)?.trim_start_matches(','));
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

/// Literals of a different JSON type than the value they stand in for, so the reader's
/// `expected a string` / `expected a number` / `expected a boolean` paths are reached.
const SWAPS: [&str; 6] = ["null", "true", "0", "1.5", r#""""#, r#""x""#];

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

/// The span of an `id` string's contents, for the two edits that touch ids.
fn id_spans(text: &str) -> Vec<(usize, usize)> {
    spots(text, r#""id":""#)
        .into_iter()
        .filter_map(|start| {
            let value = start + 6;
            let close = text.get(value..)?.find('"')? + value;
            Some((value, close))
        })
        .collect()
}

/// One `id` given another's value. The target is drawn (not always the last) so the
/// duplicate lands on a node and on an edge across the corpus, and both refusal texts
/// (`DuplicateId`) are reached.
fn duplicate_id(text: &str, rng: &mut Rng) -> Option<String> {
    let spans = id_spans(text);
    let first = *spans.first()?;
    let target = if spans.len() == 1 {
        first
    } else {
        let at = rng.below(spans.len());
        if at == 0 { spans[1] } else { spans[at] }
    };
    let borrowed = text.get(first.0..first.1)?.to_owned();
    let mut out = String::with_capacity(text.len());
    out.push_str(&text[..target.0]);
    out.push_str(&borrowed);
    out.push_str(&text[target.1..]);
    Some(out)
}

/// An endpoint pointed at an id nothing else carries, so the dangling check is reached.
fn rename_endpoint(text: &str, rng: &mut Rng) -> Option<String> {
    let ends = [r#""source":""#, r#""target":""#];
    let key = ends[rng.below(ends.len())];
    let start = *spots(text, key).last()?;
    let value = start + key.len();
    let close = text.get(value..)?.find('"')? + value;
    let mut out = String::with_capacity(text.len() + 8);
    out.push_str(&text[..value]);
    out.push_str("ghost");
    out.push_str(&rng.below(1000).to_string());
    out.push_str(&text[close..]);
    Some(out)
}
