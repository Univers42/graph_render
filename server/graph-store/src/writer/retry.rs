//! §5.1's one retry, and nothing else.
//!
//! There is exactly one retry in this crate and this file is all of it. Three SQLSTATEs qualify,
//! and the macro that runs the attempt a second time is here so the next writer does not write a
//! second one.

use crate::error::StoreError;

/// Is this refusal the one the writer runs the whole transaction again for?
///
/// `40P01` and `40001` are the deadlock and the serialization failure that manual SQL can cause
/// against the step-1 lock order (§5.3: an `UPDATE records` locks rows, then its trigger locks the
/// workspace row, the reverse of §5.1's order). `23505` on the idempotency key is defensive — the
/// step-1 lock makes it unreachable for two batches of one workspace, and the retry then finds the
/// stored response at step 2, which is the whole reason to have one.
///
/// Caveat: the retry is spent on the *first* failure, not on the first kind of failure, so a writer
/// that is genuinely contended fails after two attempts rather than after a backoff. The hub answers
/// 503 with `Retry-After: 1` and owns the retry budget from there: the store cannot sleep on a
/// lock without pinning a connection out of a pool of eight.
pub(crate) fn retryable(error: &StoreError) -> bool {
    match error {
        StoreError::Serialization { .. } | StoreError::Duplicate { .. } => true,
        _ => false,
    }
}

/// Run `$attempt` once, and once more after a refusal [`retryable`] allows.
///
/// Written as a macro so the attempt is the caller's own code rather than a boxed future: the whole
/// transaction is written inline at the call site, and the retry is a second evaluation of the same
/// expression on the same connection — which is exactly what §5.1's retry means. A second
/// retryable refusal is [`StoreError::Serialization`] with `retried: true`, the hub's 503.
///
/// Caveat: `$attempt` is evaluated twice, so it must not carry a side effect outside the
/// transaction. Every call site is a `BEGIN … COMMIT` on `$client`, and the only effect that
/// survives a failure is [`crate::store::Store::note_retry`] — which is the point of asserting on
/// `retry_count()`.
macro_rules! retried {
    ($store:expr, $client:expr, $attempt:expr) => {{
        let store: &$crate::store::Store = $store;
        match $attempt {
            Ok(value) => Ok(value),
            Err(error) if !$crate::writer::retry::retryable(&error) => Err(error),
            Err(_) => {
                store.note_retry();
                // The rollback is best-effort: the failure may have aborted the transaction by
                // itself, and a connection whose transaction state is unknown is caught by the
                // attempt's own first statement failing loudly rather than by a silent success.
                let _ = $client.batch_execute("ROLLBACK").await;
                match $attempt {
                    Ok(value) => Ok(value),
                    Err(error) if $crate::writer::retry::retryable(&error) => {
                        Err(crate::error::StoreError::Serialization { retried: true })
                    }
                    Err(error) => Err(error),
                }
            }
        }
    }};
}

pub(crate) use retried;