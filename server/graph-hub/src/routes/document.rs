//! The store's `Document` as a response body: one `Bytes` per piece, produced lazily, bounded by
//! §6's `STREAM_DEADLINE_MS`.
//!
//! `/graph` and the `/layout` relay both need this and only this, which is what makes "the two
//! routes cannot stream different bytes" a property of the code rather than of two implementations
//! agreeing. The `Document` itself is never concatenated: the walk yields `head()`, then one chunk
//! per `next()`, then `tail()`, and dropping the walk drops the `Document`, which is what closes the
//! store's transaction.
//!
//! Caveat: the deadline is on the **whole** stream, not per chunk, so a client reading steadily but
//! slowly is cut at `STREAM_DEADLINE_MS` from the first byte. That is §6's row and the reason
//! `hub-memory` measures what a slow reader costs.

use std::time::Duration;

use axum::body::Bytes;
use futures_util::stream::{self, Stream};
use graph_store::materialize::Document;

/// Where the walk is in the document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stage {
    /// `head()` has not been yielded.
    Head,
    /// At least one `next()` is owed.
    Body,
    /// `next()` returned `None`, so `tail()` is owed.
    Tail,
    /// Nothing is owed; the stream is over.
    Done,
}

/// What one turn of the walk produced.
#[derive(Debug)]
enum Step {
    /// A piece of the document.
    Piece(Bytes),
    /// The stage moved but no bytes are owed yet; the walk turns again immediately.
    Again,
    /// The document is over, the deadline passed, or the store faulted.
    End,
}

/// What the walk holds: the store's document, the stage, the deadline, and the permit.
///
/// `_permit` is read by nobody and dropped with the walk on purpose: a `READS` permit must be held
/// for as long as the bytes are, and a permit freed when the response head was written would let two
/// materializations run at once under one cap.
pub struct Walk {
    document: Option<Document>,
    stage: Stage,
    deadline: tokio::time::Instant,
    _permit: Option<tokio::sync::OwnedSemaphorePermit>,
}

impl Walk {
    /// A walk over `document`, held by `permit`, cut at `deadline` from now.
    pub fn new(
        document: Document,
        permit: tokio::sync::OwnedSemaphorePermit,
        deadline: Duration,
    ) -> Self {
        Walk {
            document: Some(document),
            stage: Stage::Head,
            deadline: tokio::time::Instant::now() + deadline,
            _permit: Some(permit),
        }
    }

    /// One turn of the walk, looping over the stage changes that owe no bytes.
    ///
    /// The loop is bounded by the stage machine: `Again` is only ever returned by the
    /// records-to-tail transition, so a walk makes at most one extra turn per document.
    async fn step(&mut self) -> Step {
        loop {
            let step = match self.stage {
                Stage::Head => {
                    self.stage = Stage::Body;
                    self.head()
                }
                Stage::Body => self.body().await,
                Stage::Tail => {
                    self.stage = Stage::Done;
                    self.tail()
                }
                Stage::Done => Step::End,
            };
            if matches!(step, Step::Again) {
                continue;
            }
            return step;
        }
    }

    /// `head()`, which the `Document` already holds and which needs no read.
    fn head(&mut self) -> Step {
        match self.document.as_ref() {
            Some(document) => Step::Piece(Bytes::from(document.head().to_owned())),
            None => Step::End,
        }
    }

    /// One `next()`, and the switch to [`Stage::Tail`] on the `None` that ends the records.
    async fn body(&mut self) -> Step {
        let Some(document) = self.document.as_mut() else {
            return Step::End;
        };
        match tokio::time::timeout_at(self.deadline, document.next()).await {
            Ok(Ok(Some(text))) => Step::Piece(Bytes::from(text)),
            // `None` is the end of the records: the transaction commits inside `next()` on this
            // call, and the tail closes the document.
            Ok(Ok(None)) => {
                self.stage = Stage::Tail;
                Step::Again
            }
            // A store fault or the deadline: the stream ends and the `Document` is dropped with the
            // walk, which rolls the snapshot back — the right outcome for a client that is gone.
            Ok(Err(_)) | Err(_) => {
                self.stage = Stage::Done;
                Step::End
            }
        }
    }

    /// `tail()`, the closing bracket of the document.
    fn tail(&mut self) -> Step {
        match self.document.as_ref() {
            Some(document) => Step::Piece(Bytes::from(document.tail())),
            None => Step::End,
        }
    }
}

/// `document` as a stream of pieces, cut at `deadline`, holding `permit` until it ends.
///
/// The item's `Err` is `std::io::Error` because that is what `Body::from_stream` wants; this walk
/// never produces one, so an error there would be hyper's own rather than the hub's.
pub fn pieces(
    document: Document,
    permit: tokio::sync::OwnedSemaphorePermit,
    deadline: Duration,
) -> impl Stream<Item = Result<Bytes, std::io::Error>> {
    let walk = Walk::new(document, permit, deadline);
    stream::unfold(walk, |mut walk| async move {
        let step = walk.step().await;
        match step {
            Step::Piece(bytes) => Some((Ok(bytes), walk)),
            Step::Again | Step::End => None,
        }
    })
}
