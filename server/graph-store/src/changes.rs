//! The change feed (spec §5.3, Task 8): the changes after a cursor, one page at a time.
//!
//! A page is read in one `REPEATABLE READ, READ ONLY` transaction: the cursor check, the headers and
//! their operations all see one snapshot, so a prune that commits between the header read and the
//! operation read cannot take operations out from under a header the page already chose. Each
//! header's stored `ops` is checked against the rows found, and a page with a mismatch is refused
//! whole rather than sent short.
//!
//! The change text is graph-contract's: `change_json` and `manifest_change_json` rebuild it from the
//! stored operations, so the store is never a second producer of canonical change text.

mod cursor;
mod page;

use graph_contract::hub::Cursor;

use crate::error::StoreError;
use crate::store::Store;

/// The reader's seams, which do nothing outside a `test-hooks` build.
const HOOKS: crate::hooks::Hooks = crate::hooks::Hooks::new();

/// One `/changes` request, already validated by the caller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangesReq {
    /// The workspace.
    pub ws: String,
    /// The position the caller has; the page starts after it.
    pub since: Cursor,
    /// The most changes one page carries.
    pub limit: u64,
    /// The most change-text bytes one page carries, except that a page always carries at least one
    /// change when there is one.
    pub max_bytes: u64,
}

/// One page of the feed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangePage {
    /// The workspace's epoch.
    pub epoch: u64,
    /// The workspace's `head_seq` in the page's snapshot.
    pub head_seq: u64,
    /// The cursor to ask from next: the last change's seq, or `since` when the page is empty.
    pub next: Cursor,
    /// The changes, in seq order.
    pub changes: Vec<Change>,
    /// The total length of the changes' texts.
    pub bytes: u64,
}

/// What a change did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeKind {
    /// A batch of upserts and deletes.
    Batch,
    /// A manifest registration or growth.
    Manifest,
}

/// One change in the feed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    /// The seq it occupies.
    pub seq: u64,
    /// The plugin it belongs to.
    pub plugin: String,
    /// When it was applied, in the wire's spelling.
    pub at: String,
    /// A batch or a manifest change.
    pub kind: ChangeKind,
    /// The records it stored, in change order.
    pub upserts: Vec<ChangeOp>,
    /// The records it removed, in change order.
    pub deletes: Vec<ChangeOp>,
    /// Its canonical text, as graph-contract writes it.
    pub text: String,
}

/// One operation of a change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeOp {
    /// The record's qualified collection.
    pub qcoll: String,
    /// The record's id.
    pub id: String,
    /// The `rev` the store assigned.
    pub rev: u64,
    /// The record's canonical text, or the empty string for a delete.
    pub text: String,
}

/// Whether a cursor can still be served.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorState {
    /// The feed after it is complete.
    Valid,
    /// It is from another epoch, past the head, or below what is kept.
    Gone,
}

/// One page of `req.ws`'s changes after `req.since`.
///
/// `Err(StoreError::Gone)` for a cursor [`cursor_state`] calls gone; `NotFound` for a missing
/// workspace; `Db` for a header whose operation count does not match its rows.
pub async fn page(store: &Store, req: &ChangesReq) -> Result<ChangePage, StoreError> {
    page::read(store, req).await
}

/// Whether `since` can be served for `ws`: valid iff it is in the workspace's epoch and
/// `low − 1 ≤ seq ≤ head_seq`, where `low` is the lowest kept seq (`head_seq + 1` when none is kept).
pub async fn cursor_state(
    store: &Store,
    ws: &str,
    since: Cursor,
) -> Result<CursorState, StoreError> {
    let client = store.client().await?;
    Ok(cursor::Bounds::read(&client, ws).await?.state(since))
}
