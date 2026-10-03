//! Admission (Verdict conditions 4 and 5): `GRAPH_WORKERS` compute slots and at most
//! `GRAPH_QUEUE` requests waiting for one. A request that finds every slot taken and the queue
//! full is a 429 at once; one that waits past its deadline is a 503. The permit a request takes
//! moves into its blocking run, so a slot frees when the run ends, not when its response does.

use crate::error::ApiError;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tokio::time::Instant;

/// The slots and the wait queue.
#[derive(Debug)]
pub struct Gate {
    slots: Arc<Semaphore>,
    waiting: AtomicUsize,
    queue: usize,
}

impl Gate {
    /// `workers` slots and room for `queue` waiters.
    pub fn new(workers: usize, queue: usize) -> Self {
        Self {
            slots: Arc::new(Semaphore::new(workers)),
            waiting: AtomicUsize::new(0),
            queue,
        }
    }

    /// A slot, now or once one frees before `deadline`.
    pub async fn admit(&self, deadline: Instant) -> Result<OwnedSemaphorePermit, ApiError> {
        if let Ok(permit) = Arc::clone(&self.slots).try_acquire_owned() {
            return Ok(permit);
        }
        let Some(_place) = self.enqueue() else {
            return Err(ApiError::busy());
        };
        let wait = Arc::clone(&self.slots).acquire_owned();
        match tokio::time::timeout_at(deadline, wait).await {
            Ok(Ok(permit)) => Ok(permit),
            Ok(Err(_)) => Err(ApiError::internal("the compute slots were closed")),
            Err(_) => Err(ApiError::timeout()),
        }
    }

    /// Slots free right now.
    pub fn free(&self) -> usize {
        self.slots.available_permits()
    }

    fn enqueue(&self) -> Option<Place<'_>> {
        let queue = self.queue;
        let room = |waiting: usize| (waiting < queue).then_some(waiting + 1);
        self.waiting
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, room)
            .ok()
            .map(|_| Place(&self.waiting))
    }
}

/// One place in the wait queue, given back when the wait ends, however it ends.
struct Place<'a>(&'a AtomicUsize);

impl Drop for Place<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}
