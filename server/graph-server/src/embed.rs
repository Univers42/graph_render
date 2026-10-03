//! `GET /embed/<h>/*` (Verdict condition 10): the studio bundle, the SDK and the wasm, public
//! and immutable. `$GRAPH_EMBED_DIR/VERSION` holds `<h>\n` (16 lowercase hex digits) and the
//! files live in `$GRAPH_EMBED_DIR/<h>/`. The tree is read once at start, without following a
//! symlink and skipping every dotfile, and a request is a lookup in that table: a path that is
//! not a key (`..`, an encoded traversal, a directory, a dotfile, a symlink, `VERSION` itself)
//! cannot reach the disk, so it is a 404.

use crate::app::App;
use crate::breaks;
use crate::error::ApiError;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderValue, Uri, header};
use axum::response::{IntoResponse, Response};
use percent_encoding::percent_decode_str;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// The most the embed tree may hold, read into memory at start. Caveat: a ceiling picked well
/// above the measured bundle (a few MiB of wasm and JS), not a measurement; a larger tree is
/// refused at start rather than served from disk.
const MAX_TREE_BYTES: u64 = 256 << 20;

/// The headers every embed file carries (condition 10). COOP and COEP only matter on a page,
/// but they cost nothing on a script and the threads build needs them on its worker.
const HEADERS: [(&str, &str); 6] = [
    ("cache-control", "public, max-age=31536000, immutable"),
    ("x-content-type-options", "nosniff"),
    ("cross-origin-resource-policy", "cross-origin"),
    ("cross-origin-embedder-policy", "require-corp"),
    ("cross-origin-opener-policy", "same-origin"),
    ("access-control-allow-origin", "*"),
];

/// One file, in memory.
#[derive(Debug, Clone)]
struct Asset {
    bytes: Bytes,
    media: &'static str,
}

/// The versioned tree.
#[derive(Debug, Clone)]
pub struct Embed {
    version: String,
    files: BTreeMap<String, Asset>,
}

impl Embed {
    /// Reads `dir/VERSION` and the tree under `dir/<h>/`. The message is for the start refusal.
    pub fn load(dir: &Path) -> Result<Self, String> {
        let version = read_version(&dir.join("VERSION"))?;
        let root = dir.join(&version);
        let meta = std::fs::symlink_metadata(&root);
        if !meta.is_ok_and(|meta| meta.is_dir()) {
            return Err("GRAPH_EMBED_DIR/<VERSION> is not a directory".to_owned());
        }
        let mut files = BTreeMap::new();
        walk(&root, &mut files)?;
        Ok(Self { version, files })
    }

    /// The version every path is served under.
    pub fn version(&self) -> &str {
        &self.version
    }

    /// How many files are served.
    pub fn len(&self) -> usize {
        self.files.len()
    }

    /// True when the tree holds no file.
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }
}

/// The handler: a table lookup, never a filesystem path built from the request.
pub async fn serve(State(app): State<Arc<App>>, uri: Uri) -> Response {
    let found = app.embed.as_ref().and_then(|embed| {
        let rest = uri.path().strip_prefix("/embed/")?;
        let (version, path) = rest.split_once('/')?;
        let path = percent_decode_str(path).decode_utf8().ok()?;
        (version == embed.version).then(|| embed.files.get(path.as_ref()))?
    });
    let Some(asset) = found else {
        return ApiError::not_found().into_response();
    };
    let mut response = ([(header::CONTENT_TYPE, asset.media)], asset.bytes.clone()).into_response();
    for (name, value) in HEADERS {
        let value = HeaderValue::from_static(value);
        response.headers_mut().insert(name, value);
    }
    response
}

fn read_version(path: &Path) -> Result<String, String> {
    let refuse = || "GRAPH_EMBED_DIR/VERSION must hold 16 lowercase hex digits and a newline";
    let text = std::fs::read_to_string(path).map_err(|_| refuse().to_owned())?;
    let version = text.strip_suffix('\n').ok_or_else(refuse)?;
    let hex = |b: u8| b.is_ascii_digit() || (b'a'..=b'f').contains(&b);
    if version.len() == 16 && version.bytes().all(hex) {
        Ok(version.to_owned())
    } else {
        Err(refuse().to_owned())
    }
}

/// Every regular file under `root`, keyed by its `/`-joined path. Symlinks and names starting
/// with `.` are skipped at any depth.
fn walk(root: &Path, files: &mut BTreeMap<String, Asset>) -> Result<(), String> {
    let mut tree = Tree {
        pending: vec![(root.to_owned(), String::new())],
        files,
        total: 0,
    };
    while let Some((dir, prefix)) = tree.pending.pop() {
        let entries = std::fs::read_dir(&dir).map_err(|_| unreadable(&prefix))?;
        for entry in entries {
            let entry = entry.map_err(|_| unreadable(&prefix))?;
            tree.visit(&entry, &prefix)?;
        }
    }
    Ok(())
}

/// The walk's state: directories still to read, the files read and their total size.
struct Tree<'a> {
    pending: Vec<(PathBuf, String)>,
    files: &'a mut BTreeMap<String, Asset>,
    total: u64,
}

impl Tree<'_> {
    fn visit(&mut self, entry: &std::fs::DirEntry, prefix: &str) -> Result<(), String> {
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            return Ok(());
        };
        let key = format!("{prefix}{name}");
        let meta = if breaks::on("follow-symlinks") {
            std::fs::metadata(entry.path())
        } else {
            std::fs::symlink_metadata(entry.path())
        };
        let meta = meta.map_err(|_| unreadable(&key))?;
        if name.starts_with('.') || meta.file_type().is_symlink() {
            return Ok(());
        }
        if meta.is_dir() {
            self.pending.push((entry.path(), format!("{key}/")));
            return Ok(());
        }
        if !meta.is_file() {
            return Ok(());
        }
        self.total += meta.len();
        if self.total > MAX_TREE_BYTES {
            return Err("the embed tree is larger than 256 MiB".to_owned());
        }
        let bytes = std::fs::read(entry.path()).map_err(|_| unreadable(&key))?;
        let media = media_type(&name);
        let bytes = Bytes::from(bytes);
        self.files.insert(key, Asset { bytes, media });
        Ok(())
    }
}

fn unreadable(path: &str) -> String {
    format!("the embed tree cannot be read at `{path}`")
}

/// The `Content-Type` of a file by its extension; an unknown one is served as bytes.
fn media_type(name: &str) -> &'static str {
    match name.rsplit_once('.').map(|(_, extension)| extension) {
        Some("js" | "mjs") => "text/javascript",
        Some("wasm") => "application/wasm",
        Some("json" | "map") => "application/json",
        Some("css") => "text/css",
        Some("html") => "text/html; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("txt") => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}
