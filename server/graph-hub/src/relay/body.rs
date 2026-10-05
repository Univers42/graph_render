//! The store's `Document` as the relay's chunked request body, and the counters one upload keeps.
//!
//! `/graph` and `/layout` walk the same `Document` piece by piece, and this is the `/layout` half of
//! that: `head()`, then one chunk per `next()`, then `tail()`. The walk owns the `Document` and
//! drops it when the stream ends, which is what closes the store's snapshot — and because
//! `next()` returns `None` only after the store has committed, the transaction is already closed by
//! the time the last chunk reaches the socket, long before the motor's answer is awaited (§5.3).
//!
//! The `drop-record` break lives here and **only** here: `/graph` walks its own document through
//! `routes/document.rs`, so the two routes cannot change together, which is the whole point of D4
//! and of row `negctl-drop-record`.
//!
//! Caveat: the deadline is on the **whole** upload rather than per chunk, so a motor that reads
//! slowly is cut at `GRAPH_HUB_STREAM_DEADLINE_MS` from the first chunk. §5.3's own bound on the
//! exchange is `GRAPH_HUB_MOTOR_TIMEOUT_MS`, which is shorter, so this cut is the backstop.

use std::sync::atomic::{AtomicU64, Ordering};

use axum::body::{Body, Bytes};
use futures_util::stream::{self, Stream};
use graph_store::materialize::Document;
use http_body_util::BodyStream;
use tokio::sync::OwnedSemaphorePermit;

use crate::breaks;

/// What one upload counted. A test reads this instead of RSS, so the claim "no whole document is
/// ever in memory" is a number the relay itself produced.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Counts {
    /// Chunks written to the request body, `head` and `tail` included.
    pub chunks: u64,
    /// The largest single chunk, in bytes.
    pub largest: u64,
    /// The most bytes the walk held at once, which is one chunk: it yields before it reads again.
    pub held: u64,
    /// Records `drop-record` skipped: 0 in a shipped build, 1 with the break on.
    pub dropped: u64,
}

/// The counters one [`document`] walk accumulates. Cheap enough to hold in a shipped relay: four
/// relaxed atomic updates per chunk, on a path that is already writing that chunk to a socket.
#[derive(Debug, Default)]
pub struct Probe {
    chunks: AtomicU64,
    largest: AtomicU64,
    held: AtomicU64,
    dropped: AtomicU64,
}

impl Probe {
    /// What this upload counted so far.
    pub fn counts(&self) -> Counts {
        Counts {
            chunks: self.chunks.load(Ordering::Relaxed),
            largest: self.largest.load(Ordering::Relaxed),
            held: self.held.load(Ordering::Relaxed),
            dropped: self.dropped.load(Ordering::Relaxed),
        }
    }

    /// One chunk of `bytes`, counted. `fetch_max` and not a compare-and-swap loop, so two of these
    /// cannot race into a smaller maximum than either saw.
    fn wrote(&self, bytes: &[u8]) {
        self.chunks.fetch_add(1, Ordering::Relaxed);
        let size = bytes.len() as u64;
        self.largest.fetch_max(size, Ordering::Relaxed);
        self.held.fetch_max(size, Ordering::Relaxed);
    }

    /// One record skipped by `drop-record`.
    fn dropped(&self) {
        self.dropped.fetch_add(1, Ordering::Relaxed);
    }
}

/// Where the walk is in the document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stage {
    /// `head()` has not been written yet.
    Head,
    /// At least one `next()` is owed.
    Body,
    /// `next()` returned `None`, so `tail()` is owed.
    Tail,
    /// Nothing is owed; the upload is over.
    Done,
}

/// What the walk holds: the document, where it is, the probe, and whether `drop-record` still owes
/// a record.
struct Walk<'a> {
    document: Option<Document>,
    stage: Stage,
    deadline: tokio::time::Instant,
    probe: Option<&'a Probe>,
    armed: bool,
}

impl Walk<'_> {
    /// The next piece, or `None` when the document is over, the deadline passed or the store
    /// faulted. The loop is bounded by the stage machine: `drop-record` can skip one record and the
    /// `None` that ends the records moves one stage, so a walk turns at most one extra time per
    /// document plus one for the dropped record.
    async fn piece(&mut self) -> Option<Bytes> {
        loop {
            let piece = match self.stage {
                Stage::Head => {
                    self.stage = Stage::Body;
                    self.head()?
                }
                Stage::Body => match self.record().await {
                    Some(piece) => piece,
                    None => continue,
                },
                Stage::Tail => {
                    self.stage = Stage::Done;
                    self.tail()?
                }
                Stage::Done => return None,
            };
            return Some(piece);
        }
    }

    /// `head()`, which the `Document` already holds and which costs no read.
    fn head(&mut self) -> Option<Bytes> {
        let document = self.document.as_ref()?;
        let text = Bytes::from(document.head().to_owned());
        self.count(&text);
        Some(text)
    }

    /// One `next()`, with `drop-record` in it and the switch to [`Stage::Tail`] on the `None` that
    /// ends the records.
    async fn record(&mut self) -> Option<Bytes> {
        let document = self.document.as_mut()?;
        let read = tokio::time::timeout_at(self.deadline, document.next()).await;
        match read {
            Ok(Ok(Some(text))) => {
                if self.armed && breaks::on("drop-record") {
                    // Exactly one record, in the relay's stream only, so `/graph` and `/layout`
                    // disagree by one record and row `negctl-drop-record` can see it.
                    self.armed = false;
                    if let Some(probe) = self.probe {
                        probe.dropped();
                    }
                    return None;
                }
                let piece = Bytes::from(text);
                self.count(&piece);
                Some(piece)
            }
            // `None` is the end of the records, and the store has committed by the time it says so.
            Ok(Ok(None)) => {
                self.stage = Stage::Tail;
                None
            }
            // A store fault or the deadline ends the upload; dropping the walk drops the document,
            // which is the right outcome for a motor that stopped reading.
            Ok(Err(_)) | Err(_) => {
                self.stage = Stage::Done;
                None
            }
        }
    }

    /// `tail()`, the closing bracket of the document.
    fn tail(&mut self) -> Option<Bytes> {
        let document = self.document.as_ref()?;
        let piece = Bytes::from(document.tail());
        self.count(&piece);
        Some(piece)
    }

    /// One piece, counted when a probe is installed.
    fn count(&self, bytes: &Bytes) {
        if let Some(probe) = self.probe {
            probe.wrote(bytes);
        }
    }
}

/// `document` as the relay's chunked request body, cut at `deadline`.
///
/// The `Document` is dropped when the stream ends, which is what closes the store's snapshot; the
/// item's `Err` is `std::io::Error` because that is what `Body::from_stream` wants, and this walk
/// never produces one.
///
/// Caveat: a probe is optional rather than free, so the shipped relay pays for four relaxed atomic
/// updates per chunk instead of zero. A chunk is a socket write, so the updates are noise next to
/// it; the alternative was a global counter, and a global counter cannot tell one test's upload
/// from another's.
pub fn document<'a>(
    document: Document,
    probe: Option<&'a Probe>,
    deadline: tokio::time::Instant,
) -> impl Stream<Item = Result<Bytes, std::io::Error>> + 'a {
    let walk = Walk {
        document: Some(document),
        stage: Stage::Head,
        deadline,
        probe,
        armed: true,
    };
    stream::unfold(walk, |mut walk| async move {
        walk.piece().await.map(|bytes| (Ok(bytes), walk))
    })
}

/// The answer's body with `permit` held until it ends or is dropped.
///
/// A `LAYOUTS` permit must cover the bytes, not only the head: the default of 1 is what keeps a
/// waiting `/layout` to one snapshot and one pool connection (§6), and a permit freed the moment
/// the response head was written would let a second upload start while the first was still going.
///
/// Caveat: a caller that stops reading its response does not release the permit until it drops the
/// body, which hyper does when the connection closes; a caller that keeps a connection open and
/// never reads holds the permit until `GRAPH_HUB_STREAM_DEADLINE_MS` cuts the stream.
pub fn held(body: Body, permit: OwnedSemaphorePermit) -> Body {
    let inner = BodyStream::new(body);
    Body::from_stream(stream::unfold((inner, permit), |(mut body, permit)| async move {
        let frame = futures_util::StreamExt::next(&mut body).await;
        frame.map(|item| (item, (body, permit)))
    }))
}