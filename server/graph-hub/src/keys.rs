//! The credential pair the hub answers with: a key set and a grants table, held together so
//! `SIGHUP` swaps both or neither (§5.2's Grants; `docs/decisions/graph-hub.md`, Decision 7).
//!
//! One `RwLock<Arc<(KeySet, Grants)>>` is the whole concurrency story here. A request clones the
//! `Arc` once and reads a consistent pair for its whole life, so a swap that lands mid-request
//! cannot leave it with the new key and the old grants — the case
//! `a_relay_still_holds_a_key_set_across_a_sighup` pins.

use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use graph_server::keys::KeySet;

use crate::grants::Grants;

/// The pair, and the swap. A `RwLock` rather than a channel or a `OnceCell` because the swap is
/// rare (one per `SIGHUP`) and the read is one per request: a read lock is the cheapest thing that
/// still gives the pair atomicity.
#[derive(Debug)]
pub struct Keyring {
    keys_file: PathBuf,
    grants_file: PathBuf,
    pair: RwLock<Arc<Pair>>,
}

/// The pair itself: the motor crate's key set and this crate's grants.
pub type Pair = (KeySet, Grants);

impl Keyring {
    /// Loads both files, or refuses. A keys file that loads and a grants file that does not is a
    /// refusal too, not a half-loaded pair: a hub with keys and no grants answers 403 to
    /// everything and reads as an authorization bug rather than a deployment mistake (Decision 7).
    pub fn load(keys: &Path, grants: &Path) -> Result<Self, String> {
        let pair = load_pair(keys, grants)?;
        Ok(Keyring {
            keys_file: keys.to_path_buf(),
            grants_file: grants.to_path_buf(),
            pair: RwLock::new(Arc::new(pair)),
        })
    }

    /// The pair in force right now, as one `Arc` a request holds for its whole life.
    pub fn current(&self) -> Arc<Pair> {
        // A poisoned lock means a `SIGHUP` task panicked while writing. The pair is read-only, so
        // the value behind the poison is still the last good one and reading it beats serving 401s.
        Arc::clone(
            &self
                .pair
                .read()
                .unwrap_or_else(|poisoned| poisoned.into_inner()),
        )
    }

    /// Loads both files again and swaps them together. `Err` is a refusal that left the old pair in
    /// force, and the caller logs it; `Ok(false)` is a reload whose files were unchanged, so the
    /// same `Arc` stayed and every request in flight keeps the pair it already holds.
    ///
    /// The load happens into locals first and the write lock is taken only once both files are
    /// `Ok`, so a bad grants file cannot leave the new key set beside the old grants.
    ///
    /// Caveat: the `reload-keys-only` break swaps the key set and leaves the grants table as it
    /// was, which is exactly the half-swap this design forbids; row `negctl-reload-keys-only`
    /// turns it on and `a_good_pair_swaps_both` goes red.
    pub fn reload(&self) -> Result<bool, String> {
        let keys = &self.keys_file;
        let grants = &self.grants_file;
        let fresh = load_pair(keys, grants)?;
        let pair = if crate::breaks::on("reload-keys-only") {
            let old = self.current();
            Arc::new((fresh.0, old.1.clone()))
        } else {
            Arc::new(fresh)
        };
        let mut slot = self
            .pair
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if Arc::ptr_eq(&slot, &pair) {
            return Ok(false);
        }
        *slot = pair;
        Ok(true)
    }

    /// How many keys the pair holds, for a log line.
    pub fn len(&self) -> usize {
        self.current().0.len()
    }

    /// Always false: `KeySet::load` refuses an empty file, so a loaded pair holds at least one
    /// key. Named because `len` alone trips `clippy::len_without_is_empty`.
    pub fn is_empty(&self) -> bool {
        false
    }
}

/// Both files, or the first refusal. `KeySet::load`'s own error names a line number and a reason
/// and never the line itself, and the grants loader is the same about its own.
fn load_pair(keys: &Path, grants: &Path) -> Result<Pair, String> {
    let keys = KeySet::load(keys).map_err(|error| format!("key file: {error}"))?;
    let grants = Grants::load(grants)?;
    Ok((keys, grants))
}
