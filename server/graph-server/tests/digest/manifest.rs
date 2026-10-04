//! The digest manifest (`docs/contract/service-api.md` "Verdict", condition 7): the
//! `(fixture, source, layout, post, hash)` rows `svc-digest` compares the seam's bytes against.
//!
//! `hash` is the SHA-256 of the binary snapshot in lowercase hex, taken exactly as
//! `graph-cli hashgate` takes it (`crates/graph-cli/src/runner.rs` `sha256_hex`), or
//! `refused:<Code::name>` when the motor refuses that row — a refusal is evidence too, so it
//! is pinned rather than skipped.
//!
//! `render` is the only writer, and it writes the shape `parse` reads, one row per line, so a
//! hash change is a one-line diff. A row is rewritten only by the ignored `emit_the_manifest`
//! test, which needs `GM_SVC_DIGEST_EMIT=1` as well: a plain run must never edit the evidence
//! it is checked against.
//!
//! Caveat: the first manifest was emitted from the motor's own current output, so the native
//! arm alone is a change detector, not an oracle. `scripts/orch/svc-digest-wasm.sh` is the
//! other arm — the same rows through the real wasm32 artifact — and it is what makes a hash
//! in this file a claim about the motor rather than about itself.

use graph_wasm::service::Source;
use serde_json::Value;
use std::path::PathBuf;

/// The committed manifest.
pub const TEXT: &str = include_str!("manifest.json");

/// The manifest's path under the repository root: `render` writes it, `wasm-arm.mjs` reads
/// it, and both are called with the repository root as the working directory.
pub const PATH: &str = "server/graph-server/tests/digest/manifest.json";

/// The window a fixture's largest component must fall in, for spectral's LOBPCG branch
/// (`docs/contract/service-api.md` condition 7): 257 nodes is the smallest above the
/// degenerate block, 700 is the cap `docs/measurements/service-caps.tsv` gives that layout.
pub const COMPONENT_FLOOR: usize = 257;
/// The window's upper bound, the cap above.
pub const COMPONENT_CEILING: usize = 700;

/// One row: what to run, and the digest its bytes must hash to. No `Ord`: `Source` has none
/// and adding it to the motor for a test's sort would be a change the service does not need.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// The fixture's path, under the repository root.
    pub fixture: String,
    /// Which reader runs it.
    pub source: Source,
    /// The layout id.
    pub layout: String,
    /// The POST pass id, when the row runs one over the layout.
    pub post: Option<String>,
    /// Lowercase hex SHA-256 of the binary snapshot, or `refused:<code>`.
    pub hash: String,
}

impl Entry {
    /// The row as one tab-separated line: `fixture`, `source`, `layout`, the POST id or `-`,
    /// then the digest. `wasm-arm.mjs` prints exactly this shape, so the two arms are
    /// compared as lines and neither arm invents a format of its own.
    pub fn line(&self) -> String {
        let post = self.post.as_deref().unwrap_or("-");
        format!(
            "{}\t{}\t{}\t{}\t{}",
            self.fixture,
            name_of(self.source),
            self.layout,
            post,
            self.hash
        )
    }
}

/// The reader `name` names. Anything else is a manifest that does not match the contract's own
/// `source=` values, so it is refused rather than guessed at.
pub fn source_of(name: &str) -> Result<Source, String> {
    match name {
        "studio" => Ok(Source::Ingest),
        "contract" => Ok(Source::Contract),
        other => Err(format!("`source` is `{other}`, neither `studio` nor `contract`")),
    }
}

/// The manifest's name for `source`: the `source=` query value, not the enum variant.
pub fn name_of(source: Source) -> &'static str {
    match source {
        Source::Ingest => "studio",
        Source::Contract => "contract",
    }
}

/// Every row of the committed manifest, in file order. A malformed manifest panics: a digest
/// test that quietly skipped the rows it could not read would pass on nothing.
pub fn entries() -> Vec<Entry> {
    parse(TEXT).unwrap_or_else(|refused| panic!("{PATH}: {refused}"))
}

/// The rows to emit: the committed ones, in order, with a missing `hash` read as empty rather
/// than refused. Only the ignored emitter reads a manifest that has no digests in it yet —
/// that is the state it exists to leave. `entries` stays strict, so a hash dropped from a
/// committed manifest is still a failure and not a row waiting to be refilled.
pub fn rows_to_emit() -> Vec<Entry> {
    let value: Value =
        serde_json::from_str(TEXT).unwrap_or_else(|error| panic!("{PATH}: not JSON: {error}"));
    let rows = value["entries"]
        .as_array()
        .unwrap_or_else(|| panic!("{PATH}: no `entries` array at the top level"));
    rows.iter()
        .enumerate()
        .map(|(at, row)| row_of(row, at, false))
        .collect::<Result<Vec<Entry>, String>>()
        .unwrap_or_else(|refused| panic!("{PATH}: {refused}"))
}

/// The rows of `text`, in order, or the refusal naming the row at fault.
pub fn parse(text: &str) -> Result<Vec<Entry>, String> {
    let value: Value = serde_json::from_str(text).map_err(|error| format!("not JSON: {error}"))?;
    let rows = value["entries"]
        .as_array()
        .ok_or("no `entries` array at the top level")?;
    rows.iter()
        .enumerate()
        .map(|(at, row)| row_of(row, at, true))
        .collect()
}

/// One row of `text`. A refusal names the row by its line in the file, so a bad manifest says
/// which line to look at. `strict` refuses a row with no `hash`; see [`rows_to_emit`].
fn row_of(row: &Value, at: usize, strict: bool) -> Result<Entry, String> {
    let refuse = |what: String| format!("row {}: {what}", at + 1);
    let text = |field: &str| {
        row[field]
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| refuse(format!("no `{field}` string")))
    };
    let post = match row["post"] {
        Value::Null => None,
        ref value => Some(
            value
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| refuse("`post` is neither null nor a string".to_owned()))?,
        ),
    };
    let source = text("source").and_then(|name| source_of(&name).map_err(&refuse));
    let hash = match row["hash"].as_str() {
        Some(hash) => hash.to_owned(),
        None if strict => return Err(refuse("no `hash` string".to_owned())),
        None => String::new(),
    };
    Ok(Entry {
        fixture: text("fixture")?,
        source: source?,
        layout: text("layout")?,
        post,
        hash,
    })
}

/// The file `entries` writes: the shape `parse` reads, one row per line, in the given order.
pub fn render(entries: &[Entry]) -> String {
    let mut out = String::from("{\n  \"entries\": [\n");
    for (at, entry) in entries.iter().enumerate() {
        if at > 0 {
            out.push_str(",\n");
        }
        out.push_str("    ");
        out.push_str(&fields(entry));
    }
    out.push_str("\n  ]\n}\n");
    out
}

/// One rendered row, its strings quoted and escaped by `serde_json`.
fn fields(entry: &Entry) -> String {
    let quote = |text: &str| serde_json::to_string(text).expect("a string is quotable");
    let post = entry.post.as_deref().map_or_else(|| "null".to_owned(), quote);
    format!(
        r#"{{"fixture":{},"source":{},"layout":{},"post":{},"hash":{}}}"#,
        quote(&entry.fixture),
        quote(name_of(entry.source)),
        quote(&entry.layout),
        post,
        quote(&entry.hash),
    )
}

/// The repository root, from the test binary's own manifest directory: the manifest and the
/// fixtures are named relative to it, as `wasm-arm.mjs` names them too.
pub fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Writes `entries` over [`PATH`]. Only the ignored `emit_the_manifest` test calls this, and
/// only with `GM_SVC_DIGEST_EMIT=1`.
pub fn write(entries: &[Entry]) {
    let path = root().join(PATH);
    std::fs::write(&path, render(entries)).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
}