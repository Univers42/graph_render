//! The grants file's grammar (§5.2's Grants, verbatim) and the lookup that decides one request.
//!
//! A line is `<key-name> <ws|*> <read|write:<plugin>|admin>`, one per line; blank lines and lines
//! starting with `#` are skipped. A malformed line is a **load** failure and never a skip: a
//! grants file that half-parses is a security bug, because the line nobody read is the line nobody
//! enforces.
//!
//! The map is a `BTreeMap` and never a `HashMap`: a grants file is hashed by nothing, but it is
//! diffable and it is named in log lines, and both need a fixed order.

use graph_contract::hub::check_plugin_id;
use std::collections::BTreeMap;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use crate::breaks;

/// The longest a grants file may be, in bytes. Caveat: 1 MiB is generous for lines of at most
/// ~80 bytes, so it is a bound against a wrong file rather than a limit an operator meets.
pub const MAX_FILE_BYTES: u64 = 1 << 20;

/// The group and other bits of a mode: nothing at all for either, because a grants file is a list
/// of what a key may do and anyone who can read it can read every workspace's name.
const GROUP_AND_OTHERS: u32 = 0o037;

/// What one request needs of a key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Need<'a> {
    /// A read: any of §5.2's read routes.
    Read,
    /// A write of one plugin: `write:<plugin>` covers this plugin's writes and every read.
    Write(&'a str),
    /// An administrative write: a workspace create.
    Admin,
}

/// What one grant line allows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mode {
    /// `read`: this workspace's reads, and nothing else.
    Read,
    /// `write:<plugin>`: this plugin's writes and this workspace's reads.
    Write(String),
    /// `admin`: every write on this workspace and every read.
    Admin,
}

/// One grant line, with the workspace it names or `*`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Grant {
    /// The workspace, or `*` for every workspace.
    pub ws: String,
    /// What the line allows.
    pub mode: Mode,
}

/// Every grant in the file, keyed by the key's **name** (never its hash: the hub never learns a key,
/// only which name presented it).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Grants {
    map: BTreeMap<String, Vec<Grant>>,
}

impl Grants {
    /// Every grant of one key, in file order. `BTreeMap` order, then insertion order inside a key,
    /// so a decision is never taken from a hash's iteration order.
    pub fn of(&self, key: &str) -> &[Grant] {
        self.map.get(key).map_or(&[], Vec::as_slice)
    }

    /// Reads and parses the file. Only 0640 or stricter is accepted, the same check the keys file
    /// gets (`server/graph-server/src/keys.rs:74`) and the one §5.2 requires here.
    pub fn load(path: &Path) -> Result<Self, String> {
        use std::io::Read;
        let whole = |reason: &str| format!("grants file: {reason}");
        let file = std::fs::File::open(path).map_err(|_| whole("cannot be opened"))?;
        let meta = file.metadata().map_err(|_| whole("cannot be read"))?;
        if !meta.is_file() {
            return Err(whole("is not a regular file"));
        }
        if meta.permissions().mode() & GROUP_AND_OTHERS != 0 {
            return Err(whole("is not 0640 or stricter"));
        }
        let mut bytes = Vec::new();
        let read = file.take(MAX_FILE_BYTES + 1).read_to_end(&mut bytes);
        read.map_err(|_| whole("cannot be read"))?;
        if bytes.len() as u64 > MAX_FILE_BYTES {
            return Err(whole("is larger than 1 MiB"));
        }
        let text = String::from_utf8(bytes).map_err(|_| whole("is not UTF-8"))?;
        Self::parse(&text).map_err(|error| format!("grants file: {error}"))
    }

    /// Parses grants-file text.
    ///
    /// An empty file is a load failure: a hub with keys and no grants answers 403 to everything
    /// and reads as an authorization bug rather than a deployment mistake (Decision 7).
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut map: BTreeMap<String, Vec<Grant>> = Default::default();
        let mut lines = 0usize;
        for (index, raw) in text.split('\n').enumerate() {
            let Some(grant) =
                parse_line(raw).map_err(|why| format!("line {}: {why}", index + 1))?
            else {
                continue;
            };
            map.entry(grant.0).or_default().push(grant.1);
            lines += 1;
        }
        if lines == 0 {
            return Err(String::from("holds no grant"));
        }
        Ok(Grants { map })
    }

    /// Does `key` hold a grant covering this workspace and this need?
    ///
    /// `admin` covers everything, `read` covers reads only, `write:<plugin>` covers that plugin's
    /// writes and every read. The scan is over one key's few lines, in file order, so the first
    /// line that covers it decides — a file with two contradictory lines for one key is the
    /// operator's, and the earlier line is the one in force.
    ///
    /// The `skip-grant` break returns `true` whatever it read, which is what row
    /// `negctl-skip-grant` forces: every 403 in the matrix goes green and
    /// `no_grant_is_403_before_any_404` fails.
    pub fn allows(&self, key: &str, ws: &str, need: Need<'_>) -> bool {
        if breaks::on("skip-grant") {
            return true;
        }
        self.of(key).iter().any(|grant| grant.covers(ws, need))
    }
}

impl Grant {
    /// Does this one line cover `ws` and `need`?
    ///
    /// The workspace first, then the mode, and the mode half is `auth::grant::covers` so there is
    /// one copy of those rules and not two that can disagree.
    fn covers(&self, ws: &str, need: Need<'_>) -> bool {
        (self.ws == "*" || self.ws == ws) && crate::auth::grant::covers(&self.mode, need)
    }
}

/// One line into `(name, grant)`, or `None` for a blank or comment line.
fn parse_line(raw: &str) -> Result<Option<(String, Grant)>, String> {
    let line = raw.trim();
    if line.is_empty() || line.starts_with('#') {
        return Ok(None);
    }
    let mut fields = line.split_whitespace();
    let name = fields.next().unwrap_or_default();
    check_name(name)?;
    let ws = fields
        .next()
        .ok_or_else(|| String::from("expected <key> <ws|*> <mode>"))?;
    if ws != "*" {
        graph_contract::hub::check_workspace_id(ws).map_err(|error| error.to_string())?;
    }
    let mode = fields
        .next()
        .ok_or_else(|| String::from("expected <key> <ws|*> <mode>"))?;
    if fields.next().is_some() {
        return Err(String::from("expected <key> <ws|*> <mode>"));
    }
    Ok(Some((
        name.to_owned(),
        Grant {
            ws: ws.to_owned(),
            mode: parse_mode(mode)?,
        },
    )))
}

/// `read`, `write:<plugin>` or `admin`.
fn parse_mode(text: &str) -> Result<Mode, String> {
    if text == "admin" {
        return Ok(Mode::Admin);
    }
    if text == "read" {
        return Ok(Mode::Read);
    }
    let Some(plugin) = text.strip_prefix("write:") else {
        return Err(String::from("expected read, write:<plugin> or admin"));
    };
    check_plugin_id(plugin).map_err(|error| error.to_string())?;
    Ok(Mode::Write(plugin.to_owned()))
}

/// A key name: 1–64 of `[A-Za-z0-9._-]`, the keys file's own rule
/// (`server/graph-server/src/keys.rs:194-211`), so one name is one name in both files.
fn check_name(name: &str) -> Result<(), String> {
    let legal = !name.is_empty()
        && name.len() <= 64
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'));
    if legal {
        return Ok(());
    }
    Err(String::from("the key name is not 1-64 of [A-Za-z0-9._-]"))
}
