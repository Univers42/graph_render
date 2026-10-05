//! One read of change **headers** for a subscriber: at most `GRAPH_HUB_SSE_PAGE` of them, in seq
//! order, and nothing but the three fields a notice carries.
//!
//! The read is the store's own [`graph_store::changes::page`] with a limit, so the visibility order
//! §5.3 rests on is the store's and not a second query here. What this module adds is the shape: a
//! [`Page`] of [`Head`]s, each of which is exactly what `notice_json` takes.

use graph_contract::hub::{ChangeHead, Cursor};
use graph_store::changes::{ChangesReq, page as changes_page};
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
    /// The most bytes this read may return, §6's `CHANGES_BYTES`.
    pub max_bytes: u64,
}

/// One header, the whole of what a notice says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Head {
    /// The change's seq, which is also the notice's `id:`.
    pub seq: u64,
    /// The plugin the change belongs to.
    pub plugin: String,
    /// The change's timestamp, RFC 3339 in UTC as the store wrote it.
    pub at: String,
}

impl Head {
    /// The graph-contract notice this header is, as its three fields.
    ///
    /// WHY a conversion and not the store's own type: `notice_json` takes a `ChangeHead`, and
    /// building one here keeps graph-contract the only producer of the notice text.
    pub fn as_change_head(&self) -> ChangeHead<'_> {
        ChangeHead {
            seq: self.seq,
            plugin: &self.plugin,
            at: &self.at,
        }
    }
}

/// One page of headers and the cursor the next read starts from.
#[derive(Debug, Clone)]
pub struct Page {
    /// The workspace's epoch, which a heartbeat re-reads to notice a promotion.
    pub epoch: u64,
    /// The workspace's `head_seq` when the page was read.
    pub head_seq: u64,
    /// Where the next read starts: the last seq this one sent, or `since` when it sent none.
    pub next: Cursor,
    /// The headers, in seq order.
    pub heads: Vec<Head>,
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
    let answer = changes_page(
        store,
        &ChangesReq {
            ws: req.ws.clone(),
            since: req.since,
            limit: crate::config::capped(req.at_most),
            max_bytes: crate::config::capped(req.max_bytes),
        },
    )
    .await?;
    let heads = answer
        .changes
        .iter()
        .map(|change| Head {
            seq: change.seq,
            plugin: change.plugin.clone(),
            at: change.at.clone(),
        })
        .collect();
    Ok(Page {
        epoch: answer.epoch,
        head_seq: answer.head_seq,
        next: answer.next,
        heads,
    })
}