//! One read of change **headers** for a subscriber: at most `GRAPH_HUB_SSE_PAGE` of them, in seq
//! order, and nothing but the three fields a notice carries.
//!
//! The read is the store's [`graph_store::changes::heads`]: the same snapshot, cursor check and cuts
//! as a `/changes` page, without reading one operation, so a subscriber holds §6's
//! `SSE_PAGE × max_header` and the visibility order §5.3 rests on is still the store's.

pub use graph_store::changes::{Head, HeadPage as Page};

use graph_contract::hub::Cursor;
use graph_store::changes::{ChangesReq, heads, page as full_page};
use graph_store::{Store, StoreError};

/// One read's request: the workspace, the cursor to resume after, and how many headers at most.
#[derive(Debug, Clone)]
pub struct PageReq {
    /// The workspace whose log is read.
    pub ws: String,
    /// The cursor the subscriber last saw; the page carries the changes **after** it.
    pub since: Cursor,
    /// The most headers this read may return, at most §6's `SSE_PAGE`.
    pub at_most: u64,
    /// The most stored change bytes this read may cover, §6's `CHANGES_BYTES`.
    pub max_bytes: u64,
}

/// Read one page of headers after `req.since`.
///
/// A `StoreError::Gone` is passed through rather than mapped: the caller turns it into the
/// `resync` that ends the stream, and a mapping here would hide which of §5.3's three bounds the
/// cursor fell outside.
///
/// Caveat: the header count is the caller's number, not a re-read of the settings, so a test that
/// wants a smaller page than §6's 256 gets one without a settings round trip; the route passes
/// `settings.sse_page` and the byte cap comes from the same `Settings` the start check read.
pub async fn page(store: &Store, req: &PageReq) -> Result<Page, StoreError> {
    let changes = ChangesReq {
        ws: req.ws.clone(),
        since: req.since,
        limit: crate::config::capped(req.at_most),
        max_bytes: crate::config::capped(req.max_bytes),
    };
    if crate::breaks::on("sse-full-page") {
        return heads_of_full_page(store, &changes).await;
    }
    heads(store, &changes).await
}

/// The `sse-full-page` break: the headers cut out of a whole `/changes` page, operations and texts
/// read and dropped, which is what a subscriber held before the header-only read.
async fn heads_of_full_page(store: &Store, req: &ChangesReq) -> Result<Page, StoreError> {
    let answer = full_page(store, req).await?;
    let heads = answer.changes.into_iter().map(|change| Head {
        seq: change.seq,
        plugin: change.plugin,
        at: change.at,
    });
    Ok(Page {
        epoch: answer.epoch,
        head_seq: answer.head_seq,
        next: answer.next,
        heads: heads.collect(),
    })
}
