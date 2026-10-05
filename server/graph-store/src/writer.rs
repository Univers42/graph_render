//! The writer transaction: the one explicit transaction every hub write runs in (spec §5.1).
//!
//! §5.1's eight steps live in three places and nowhere else: [`space`] for a workspace create,
//! [`manifest`] for a manifest PUT, [`apply`] for a batch. The four steps all three share — the
//! writer guard, the workspace row lock, the seq bump and step 8's watermark — are [`step`]. No
//! other module in this crate writes.
//!
//! Every write path opens with `set_config('hub.writer', '1', true)`, which is what keeps Task 5's
//! `ENABLE ALWAYS` triggers quiet for the hub's own writes and firing for everybody else's.

pub mod apply;
pub mod bytes;
pub mod change;
pub mod idempotency;
pub mod links;
pub mod manifest;
pub mod retry;
pub mod space;
pub mod step;

use graph_contract::hub::{Batch, Growth, Limits, Manifest};

use crate::error::StoreError;
use crate::store::Store;

/// The seams, one value for the whole crate.
///
/// WHY a module constant rather than a field on [`Store`]: every seam is a file handshake under
/// `$GM_HUB_STEP_DIR`, so a `Hooks` carries no state and there is nothing to configure. A field
/// would be a value that is always the same, and a second way to say which seams exist.
const HOOKS: crate::hooks::Hooks = crate::hooks::Hooks::new();

/// §4's manifest PUT: the plugin, the manifest to register, and the caps the request was read
/// against.
///
/// The manifest is a parsed value rather than body text: the hub has already read it with
/// graph-contract's own reader, and the store never re-reads a body it did not receive.
#[derive(Debug, Clone)]
pub struct ManifestWrite {
    /// The workspace the plugin belongs to.
    pub ws: String,
    /// The plugin's id, checked against the slug grammar before the transaction opens.
    pub plugin: String,
    /// The manifest being registered.
    pub manifest: Manifest,
    /// The caps this request was read under.
    pub limits: Limits,
}

/// What a manifest PUT did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ManifestWritten {
    /// `201` when the plugin was registered, `200` otherwise.
    pub status: u16,
    /// The seq this PUT took, or the workspace's current `head_seq` when it took none.
    pub seq: u64,
    /// Whether the manifest was unchanged, or grew. `Grown` is the only case that took a seq.
    pub growth: Growth,
}

/// The `Idempotency-Key` header, with the SHA-256 the hub computed over the raw request body.
///
/// WHY the store takes the hash rather than the body: the store is handed a parsed
/// [`Batch`], and two different bodies can parse to the same batch (a member in another order, a
/// space). §5.1's rule is about *bodies*, so the hash has to be taken where the body was.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Idempotency {
    /// The key, at most 128 bytes (§5.1).
    pub key: String,
    /// The SHA-256 of the request body, as `sha2` computed it.
    pub body_sha256: [u8; 32],
}

/// §5.1's batch: everything the transaction needs, and nothing it can read from the database.
#[derive(Debug, Clone)]
pub struct BatchWrite {
    /// The workspace the plugin belongs to.
    pub ws: String,
    /// The plugin the batch was sent to; never from the body (§5.1).
    pub plugin: String,
    /// The manifest the batch was validated against.
    ///
    /// WHY the request carries it instead of the store reading it back: the hub holds exactly one
    /// view of a plugin's declaration, and re-reading the stored text on every batch would let the
    /// two disagree inside one request. The stored row is still read for `plugin_seq` and
    /// `plugin_bytes`, which are facts about the workspace and not about the request.
    pub manifest: Manifest,
    /// What to store and what to remove.
    pub batch: Batch,
    /// The idempotency key, when the request carried one.
    pub idem: Option<Idempotency>,
    /// `If-Match: "<epoch>.<plugin_seq>"`, when the request carried one.
    pub if_match: Option<graph_contract::hub::Cursor>,
    /// The caps this request was read under.
    pub limits: Limits,
}

/// What a batch did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchOutcome {
    /// The seq the batch took, or the workspace's current `head_seq` when every operation was a
    /// no-op (§5.1).
    pub seq: u64,
    /// How many operations changed something. `0` is the honest answer for an identical resend.
    pub applied: u64,
    /// The canonical answer text, `answer_json(seq, applied)`.
    pub response: String,
}

/// §4's workspace create. See [`Store::create_workspace`], which is the door callers use.
pub(crate) async fn create_workspace(
    store: &Store,
    ws: &str,
    limits: &Limits,
) -> Result<bool, StoreError> {
    space::create(store, ws, limits).await
}

/// §4's manifest PUT. See [`Store::put_manifest`].
pub(crate) async fn put_manifest(store: &Store, req: &ManifestWrite) -> Result<ManifestWritten, StoreError> {
    manifest::put(store, req).await
}

/// §5.1's batch. See [`Store::apply_batch`].
pub(crate) async fn apply_batch(store: &Store, req: &BatchWrite) -> Result<BatchOutcome, StoreError> {
    apply::batch(store, req).await
}