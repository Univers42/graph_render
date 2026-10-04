//! Provisional ingest (`docs/contract/wasm-abi.md` "Ingest — PROVISIONAL", C13): a
//! versioned JSON array of node/edge records in the shape `graph_core::records` already
//! uses, read as strict RFC 8259 by [`scan`] — a walk that refuses exactly what
//! `graph_contract::canonical_json::parse` refuses, at the same byte offsets, and builds no
//! `Value` tree to do it.
//!
//! **The real ingest contract lives in [`crate::contract`]**, read by
//! `gm_build_contract`. This one is unchanged and stays: it is what the host studio and
//! the hash gate's C20 stage already speak, so replacing it would move a published ABI's
//! meaning. It is deliberately narrow: every member is named and
//! required (a present `null` where a field may be absent, never an omitted key) except an
//! edge's `child_first`, which version 1 reads as `false` when omitted (F-01: the SDK smoke
//! harness and the documented example omit it, so requiring it is a version 2), an
//! unknown member refuses the whole document (so a stray `hasNote` — the oracle's own
//! camelCase — is refused loudly, not silently ignored), and `kind` strings are matched
//! by exact name (`NodeKind`/`EdgeKind::from_name`), never the lossy `edge_kind_from_type`
//! heuristic. Duplicate ids and dangling edges are refused outright (C12): the
//! alternative graph_core::index_model itself takes — first-wins, drop and forget — would
//! make ingest order diverge silently from snapshot order, which is exactly the identity
//! this ABI promises callers (`docs/contract/wasm-abi.md` "Column order").

use graph_core::{EdgeRecord, NodeRecord};
// The two kind names are named only by this module's tests: `element.rs` imports its own,
// so the wasm32 release build has no user for them and an unconditional import warns there.
#[cfg(test)]
use graph_core::{EdgeKind, NodeKind};

mod at;
// Only `gm_build_columns` reads it, and the exports are wasm32-only (C21 in `lib.rs`) — but
// `service::extend_columns` is the native façade's, and the force-gate's columns arm and
// `tick --path columns` call that natively, so this module is ungated for the same reason
// `contract` and `errors` are (C21 in `lib.rs`).
pub mod columns;
mod element;
mod ids;
/// Per-phase linear-memory marks, for the ingest scale measurement only. Compiled out of
/// the default artifact, so every `mark` call site stays unconditional.
#[cfg(any(test, feature = "probe"))]
pub mod phases;
mod refusal;
mod scan;
use at::At;
pub use ids::index;
#[cfg(any(test, feature = "probe"))]
use phases::mark;
pub use refusal::IngestError;

/// The only ingest version this reader accepts.
pub const VERSION: u32 = 1;

/// The longest ingest document `read` accepts, in bytes: one past this is
/// [`IngestError::TooLarge`], checked before `read` parses or `from_utf8` touches a byte.
///
/// Measured, then rounded down to a whole power of two (`docs/decisions/wasm-ingest-limits.md`
/// step 4, `docs/measurements/fix-ingest-scale.md`): the studio's own generator on the
/// `wasm32-unknown-unknown` release artifact under Node, at its 1M-node scale target and
/// upward. The largest document that built is 1,499,403,588 bytes — 1M nodes, 7,999,936
/// edges — at a peak of 4,117,561,344 of the 4,294,967,296 bytes wasm32 can address. The
/// next one up, 1,663,576,802 bytes and 8,999,919 edges, trapped while the records were being
/// read. `2^30` = 1,073,741,824 is the largest power of two at or below the largest that
/// built, and it refuses nothing that built before the reader stopped building a `Value`
/// tree — the previous ceiling's own document, and every document under it.
///
/// Ponytail: the rounding is 425,661,764 bytes of margin, so unlike the number it replaced
/// this one *does* refuse documents that build — everything from 1,073,741,825 to
/// 1,499,403,588 bytes was measured to build and this ceiling says no. That is the trade the
/// decision record asks for now that the arena no longer traps: a number with a rule behind
/// it and room under it, in exchange for a band where the sweep's answer and the ceiling's
/// disagree. It still bounds bytes and not the work they imply: the 1M-node degree-3
/// document is 678,016,813 bytes and the degree-8 one is 1,499,403,588, both under this
/// ceiling, but build-versus-trap runs on edges, and the trap at 8,999,919 edges sits
/// 164,173,214 bytes *above* this number while a document 425,661,764 bytes under it is the
/// largest this build has held. Failing input: a document whose bytes are few and whose edge
/// count is many — the degree-9 shape at 1M nodes, 1,663,576,802 bytes, which is refused here
/// only because it is over the ceiling, and would trap rather than refuse if the ceiling were
/// lifted. Direction: refuses early on size, never on shape, and bounds nothing else. Escape
/// hatch: `fix-ingest-scale`'s measurement is the only thing that sets this number, and
/// re-running `harness/ingest-ceiling.mjs` upward is what would move it.
pub const MAX_INGEST_BYTES: usize = 1_073_741_824;

/// The byte ceiling [`read_records`] holds a document to.
///
/// Identically [`MAX_INGEST_BYTES`] everywhere except the `probe` build, which lifts it so
/// the scale sweep can watch a document the ceiling would refuse *trap* instead of be
/// refused: the phases that trap are exactly the ones no refusing run ever reaches, and
/// `MAX_INGEST_BYTES` is what the sweep is measuring a new value for. Only the length
/// check moves — every byte after it, and every refusal below it, is the same code.
///
/// **Ponytail:** a `--features probe` artifact therefore does **not** enforce the ceiling,
/// so it is not an artifact to ship or to gate on; the sweep's build/refuse rows come from
/// the default artifact, and only the per-phase mark tables come from this one. Failing
/// input: any host that loads a probe build and trusts a refusal code. Direction: lifts a
/// limit, never lowers one. Escape hatch: `not(feature = "probe")` is the only build the
/// ceiling exists in.
#[cfg(not(feature = "probe"))]
fn ceiling() -> usize {
    MAX_INGEST_BYTES
}

#[cfg(feature = "probe")]
fn ceiling() -> usize {
    // `max`, not a bare `usize::MAX`: the documented ceiling stays named in both builds,
    // so they differ in this one expression and nowhere else.
    usize::MAX.max(MAX_INGEST_BYTES)
}

/// Parse and validate `bytes` into ingest order records, or the refusal.
///
/// The length is checked first, before [`std::str::from_utf8`] and before the parser is
/// given anything: a buffer past [`MAX_INGEST_BYTES`] is refused by its size alone, so no
/// work is done on a document this module has already promised not to read (F-16).
///
/// Each node's and edge's element is read out of the document's own text by [`element`],
/// and nothing else is built: the tree this reader used to consume by value was 3.2x the
/// document at 1M nodes, which is why the walk in [`scan`] exists. The order is unchanged
/// from the tree reader, and the differential test in [`differential`] is the judge: whole
/// text validated before any shape check, root checked before any node, every node before
/// any edge, then `check_ids`. Only tests call it: `gm_build` reads through
/// [`read_records`] and [`index`], which refuse the same documents.
#[cfg(test)]
pub fn read(bytes: &[u8]) -> Result<(Vec<NodeRecord>, Vec<EdgeRecord>), IngestError> {
    let (nodes, edges) = read_records(bytes)?;
    ids::check_ids(&nodes, &edges)?;
    Ok((nodes, edges))
}

/// `read` without C12's id pass, for a caller that hands the records to [`index`], which
/// refuses the same documents.
pub fn read_records(bytes: &[u8]) -> Result<(Vec<NodeRecord>, Vec<EdgeRecord>), IngestError> {
    if bytes.len() > ceiling() {
        return Err(IngestError::TooLarge {
            bytes: bytes.len(),
            limit: ceiling(),
        });
    }
    let text = std::str::from_utf8(bytes).map_err(|_| IngestError::Utf8)?;
    // One validating walk of the whole text, which locates the root's members on the same
    // pass. No `Value` tree: that tree measured 3.2x the text at 1M nodes and is what stopped
    // a document under 1 GiB from building inside wasm32's 4 GiB
    // (`docs/measurements/fix-ingest-scale.md`).
    let document = scan::Document::new(text).map_err(IngestError::Json)?;
    #[cfg(any(test, feature = "probe"))]
    mark(phases::PARSE, None);
    // The root's own shape, before any member of it is looked at: a document whose root is
    // an array validates and locates nothing, and saying "missing member `version`" for it
    // would send a caller looking for a member it never wrote.
    if !document.is_object() {
        return Err(shape(At::ROOT, "expected an object"));
    }
    let version = version_number(&document)?;
    if version != VERSION {
        return Err(shape(
            At::list("version"),
            &format!("unsupported version {version}"),
        ));
    }
    take_array(&document, "nodes", At::list("nodes"))?;
    take_array(&document, "edges", At::list("edges"))?;
    require_only(document.members(), &["version", "nodes", "edges"])?;
    // Every node before any edge, exactly as the reader this replaced read them, and both
    // lists at the length the first walk counted — so neither `Vec` grows by doubling.
    let nodes = read_all::<NodeRecord>(&document, "nodes", At::list("nodes"))?;
    let edges = read_all::<EdgeRecord>(&document, "edges", At::list("edges"))?;
    #[cfg(any(test, feature = "probe"))]
    mark(phases::RECORDS, None);
    Ok((nodes, edges))
}

/// The root's member named `key`, or the refusal its absence is; refused at `where_at` when
/// it is there but is not an array, which is the order the reader this replaced refused in.
fn take_array<'a>(
    document: &'a scan::Document<'a>,
    key: &str,
    where_at: At,
) -> Result<&'a scan::Member, IngestError> {
    let member = document
        .member(key)
        .ok_or_else(|| shape(At::ROOT, &format!("missing member `{key}`")))?;
    if member.elements.is_none() {
        return Err(shape(where_at, "expected an array"));
    }
    Ok(member)
}

/// Every element of the root member `key`, in order, at the length the validating walk
/// counted — so the `Vec` is allocated once and never grown.
///
/// The walk locates each element's members as it reads them, so a record costs one pass
/// over its own text and none over the document after it (`table::Element`).
fn read_all<T: element::Shape>(
    document: &scan::Document<'_>,
    key: &str,
    at: At,
) -> Result<Vec<T>, IngestError> {
    let member = take_array(document, key, at)?;
    let mut out = Vec::with_capacity(member.elements.unwrap_or(0));
    let mut scan = scan::Scan::new(document.text());
    let mut element = element::Element::new(document.text(), T::FIELDS, at);
    scan.records(member.value, &mut element, &mut |element| {
        let at = element.at();
        out.push(T::read(element, at)?);
        Ok(())
    })?;
    Ok(out)
}

pub(in crate::ingest) fn shape(at: At, what: &str) -> IngestError {
    IngestError::Shape(format!("{at}: {what}"))
}

/// Refuses a member the root does not name — the strictness that turns a stray member into
/// a loud refusal instead of a silently-dropped extra. Every key the document wrote is
/// named, in document order, whether or not this reader read it.
fn require_only(members: &[scan::Member], allowed: &[&str]) -> Result<(), IngestError> {
    for member in members {
        if !allowed.contains(&member.key.as_str()) {
            return Err(shape(At::ROOT, &format!("unknown member `{}`", member.key)));
        }
    }
    Ok(())
}

/// The document's `version`: exactly a plain non-negative integer literal, never `1.0` or
/// `1e0` read loosely as `1` — a version is compared for equality, not rounded.
///
/// The member's own text, because a version is a number and not a string:
/// `{"version":"1"}` is refused for being a string, and a `version` that is not there at
/// all for being missing — both before it is compared with [`VERSION`].
fn version_number(document: &scan::Document<'_>) -> Result<u32, IngestError> {
    let at = At::list("version");
    let member = document
        .member("version")
        .ok_or_else(|| shape(At::ROOT, "missing member `version`"))?;
    let text =
        number_text(document.text(), member.value).ok_or_else(|| shape(at, "expected a number"))?;
    text.parse()
        .map_err(|_| shape(at, "expected a plain non-negative integer"))
}

/// The bytes at `span` read as a number's own text, or `None` when they are not a number.
///
/// # Precondition
///
/// `span` came from a walk that validated the document, so the bytes are one whole value
/// and this only has to recognise which kind it is: the first byte of the JSON number
/// grammar, which no string, `null`, boolean, array or object can begin with.
fn number_text(text: &str, span: scan::Span) -> Option<&str> {
    let (from, to) = span.bounds()?;
    let value = text.get(from..to)?;
    match value.as_bytes().first() {
        Some(b'-' | b'0'..=b'9') => Some(value),
        _ => None,
    }
}

#[cfg(test)]
mod differential;
#[cfg(test)]
mod tests;
