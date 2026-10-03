//! API keys (`docs/contract/service-api.md` "API keys", Verdict condition 9). The server stores
//! no key: the file holds `<name> <sha256-hex>` lines, and a presented key is hashed and held
//! against every stored hash in constant time. A refused file names the line number, never the
//! line, because a line can be a key pasted by mistake.

use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::io::Read;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::sync::{Arc, PoisonError, RwLock};
use subtle::ConstantTimeEq;

/// Every key starts with this, so a key in a log or a query is recognisable.
pub const KEY_PREFIX: &str = "gm_";
/// The random bytes in a key: 43 base64url characters after the prefix.
const KEY_BYTES: usize = 32;
/// The largest key file read; past it the file is refused rather than truncated.
const MAX_FILE_BYTES: u64 = 1 << 20;
/// The most keys one file may hold.
const MAX_KEYS: usize = 4096;
/// The longest key name.
const MAX_NAME: usize = 64;

/// A refused key file. `line` is 1-based; 0 means the file as a whole.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyFileError {
    /// The offending line, or 0.
    pub line: usize,
    /// What is wrong with it.
    pub reason: &'static str,
}

impl std::fmt::Display for KeyFileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.line {
            0 => write!(f, "key file: {}", self.reason),
            line => write!(f, "key file line {line}: {}", self.reason),
        }
    }
}

/// The parsed key file: names and the SHA-256 of their keys.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeySet {
    entries: Vec<(String, [u8; 32])>,
}

impl KeySet {
    /// Reads and parses the key file. A group- or world-writable file is refused: anyone who
    /// can write it can mint a key.
    pub fn load(path: &Path) -> Result<Self, KeyFileError> {
        let whole = |reason| KeyFileError { line: 0, reason };
        let file = std::fs::File::open(path).map_err(|_| whole("cannot be opened"))?;
        let meta = file.metadata().map_err(|_| whole("cannot be read"))?;
        if !meta.is_file() {
            return Err(whole("is not a regular file"));
        }
        if meta.permissions().mode() & 0o022 != 0 {
            return Err(whole(
                "is group- or world-writable (0640 or stricter is accepted)",
            ));
        }
        let mut bytes = Vec::new();
        let read = file.take(MAX_FILE_BYTES + 1).read_to_end(&mut bytes);
        read.map_err(|_| whole("cannot be read"))?;
        if bytes.len() as u64 > MAX_FILE_BYTES {
            return Err(whole("is larger than 1 MiB"));
        }
        let text = String::from_utf8(bytes).map_err(|_| whole("is not UTF-8"))?;
        Self::parse(&text)
    }

    /// Parses key-file text. Blank lines and lines starting with `#` are skipped.
    pub fn parse(text: &str) -> Result<Self, KeyFileError> {
        let mut entries = Vec::new();
        let (mut names, mut hashes) = (BTreeSet::new(), BTreeSet::new());
        for (index, raw) in text.split('\n').enumerate() {
            let line = index + 1;
            let refuse = |reason| KeyFileError { line, reason };
            let Some((name, hash)) = parse_line(raw).map_err(refuse)? else {
                continue;
            };
            if !names.insert(name.to_owned()) {
                return Err(refuse("duplicate name"));
            }
            if !hashes.insert(hash) {
                return Err(refuse("duplicate hash"));
            }
            if entries.len() == MAX_KEYS {
                return Err(refuse("more than 4096 keys"));
            }
            entries.push((name.to_owned(), hash));
        }
        if entries.is_empty() {
            return Err(KeyFileError {
                line: 0,
                reason: "holds no key",
            });
        }
        Ok(Self { entries })
    }

    /// The name of the key, if it is one. Every stored hash is compared, with no early exit,
    /// so a near miss is never refused faster than a far one.
    pub fn name_of(&self, key: &str) -> Option<&str> {
        let digest: [u8; 32] = Sha256::digest(key.as_bytes()).into();
        let mut found = None;
        for (name, hash) in &self.entries {
            if bool::from(hash.ct_eq(&digest)) {
                found = Some(name.as_str());
            }
        }
        found
    }

    /// How many keys the file holds.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Never true: an empty file is refused.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// The live key set, swapped whole on `SIGHUP`: a reader sees the old set or the new one,
/// never half of either.
#[derive(Debug)]
pub struct KeyStore(RwLock<Arc<KeySet>>);

impl KeyStore {
    /// A store holding `keys`.
    pub fn new(keys: KeySet) -> Self {
        Self(RwLock::new(Arc::new(keys)))
    }

    /// The current set.
    pub fn current(&self) -> Arc<KeySet> {
        Arc::clone(&self.0.read().unwrap_or_else(PoisonError::into_inner))
    }

    /// Replaces the set. The caller parses the whole file first, so a bad file never lands.
    pub fn replace(&self, keys: KeySet) {
        *self.0.write().unwrap_or_else(PoisonError::into_inner) = Arc::new(keys);
    }
}

/// One `keygen` result: the key, shown once, and its key-file line.
#[derive(Debug)]
pub struct NewKey {
    /// `gm_` + 43 base64url characters.
    pub key: String,
    /// `<name> <sha256-hex>`.
    pub line: String,
}

/// Mints a key from `/dev/urandom`. Nothing is written to disk.
pub fn keygen(name: &str) -> Result<NewKey, &'static str> {
    if !is_name(name) {
        return Err("a name is 1-64 of A-Z a-z 0-9 . _ -");
    }
    let mut random = [0u8; KEY_BYTES];
    let mut source = std::fs::File::open("/dev/urandom").map_err(|_| "cannot open /dev/urandom")?;
    source
        .read_exact(&mut random)
        .map_err(|_| "cannot read /dev/urandom")?;
    let key = format!("{KEY_PREFIX}{}", base64url(&random));
    let line = format!("{name} {}", hex(&Sha256::digest(key.as_bytes())));
    Ok(NewKey { key, line })
}

/// One non-comment line as `(name, hash)`, `None` for a blank line or a comment.
fn parse_line(raw: &str) -> Result<Option<(&str, [u8; 32])>, &'static str> {
    let line = raw.trim_matches(|c| matches!(c, ' ' | '\t' | '\r'));
    if line.is_empty() || line.starts_with('#') {
        return Ok(None);
    }
    let mut fields = line.split([' ', '\t']).filter(|field| !field.is_empty());
    let (Some(name), Some(hash), None) = (fields.next(), fields.next(), fields.next()) else {
        return Err("expected `<name> <sha256-hex>`");
    };
    if name.chars().any(char::is_control) {
        return Err("control character in the name");
    }
    if !is_name(name) {
        return Err("a name is 1-64 of A-Z a-z 0-9 . _ -");
    }
    let hash = decode_hash(hash).ok_or("the hash is not 64 hex digits")?;
    Ok(Some((name, hash)))
}

fn is_name(name: &str) -> bool {
    (1..=MAX_NAME).contains(&name.len())
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}

fn decode_hash(text: &str) -> Option<[u8; 32]> {
    let digits = text.as_bytes();
    if digits.len() != 64 {
        return None;
    }
    let mut hash = [0u8; 32];
    let (pairs, _) = digits.as_chunks::<2>();
    for (byte, pair) in hash.iter_mut().zip(pairs) {
        let pair = std::str::from_utf8(pair).ok()?;
        *byte = u8::from_str_radix(pair, 16).ok()?;
    }
    Some(hash)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// RFC 4648 §5 base64url without padding.
pub fn base64url(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let word = chunk
            .iter()
            .enumerate()
            .fold(0u32, |w, (i, b)| w | u32::from(*b) << (16 - 8 * i));
        for sextet in 0..=chunk.len() {
            out.push(char::from(
                ALPHABET[(word >> (18 - 6 * sextet) & 63) as usize],
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests;
