//! The grant rules, in one file, so `Grants::allows` and the tests read the same code.
//!
//! §5.2's table is the whole rule, and two of its rows are easy to get backwards: `admin` covers
//! everything on its workspace, and a `write:<plugin>` covers that plugin's writes **and every read**
//! of the workspace — the table lists `write:<plugin>` as an alternative grant for a records page,
//! which only makes sense if a write grant reads too.
//!
//! Caveat: `admin` is a whole-*workspace* grant and not a whole-hub one. A key granted `admin` on
//! `ops` writes and reads `ops` and learns nothing about any other workspace.

pub use crate::grants::{Grants, Mode, Need};

/// The grant lookup itself, named so `auth::authorize` reads as one call.
///
/// The lookup is on the key's **name**, from `KeySet::name_of`, the constant-time scan over every
/// stored hash (`server/graph-server/src/keys.rs:118-128`). `KeySet` has no enumerable key list,
/// which is the point: a key with no grant is denied without the hub ever learning that the key
/// exists.
pub fn allows(grants: &Grants, key: &str, ws: &str, need: &Need) -> bool {
    grants.allows(key, ws, need)
}

/// Does `mode` cover `need`, ignoring which workspace the line names?
///
/// `Grants` checks the workspace first; this answers only the mode half.
pub fn covers(mode: &Mode, need: &Need) -> bool {
    match (mode, need) {
        // `Need::Any` is answered by `Grants::allows` from the line count, so it is unreachable
        // here; the arm is `true` rather than `unreachable!()` because a panic on a public path is
        // worse than a grant that is too generous on a route no request can reach.
        (_, Need::Any) => true,
        (_, Need::Admin) => matches!(mode, Mode::Admin),
        (Mode::Admin, _) => true,
        (_, Need::Read) => true,
        (Mode::Read, Need::Write(_)) => false,
        (Mode::Write(plugin), Need::Write(wanted)) => plugin == wanted,
    }
}
