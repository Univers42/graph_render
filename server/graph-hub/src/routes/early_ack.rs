//! Row `negctl-ack-before-commit`'s break: the batch route answers **before** the store commits.
//!
//! D6 says a 200 means the change is durable. Under this break the hub computes the seq the batch
//! would get, answers it at once, and commits on a detached task five seconds later, so a
//! `docker kill` inside that window loses a batch the client was told is written. The durability
//! case `a_batch_answered_after_the_commit_survives_the_kill` must then turn red.
//!
//! Reachable only through `crate::breaks::on("ack-before-commit")`, which is a `const false`
//! without the `negctl` feature.

use std::sync::Arc;
use std::time::Duration;

use graph_store::writer::BatchWrite;

use crate::app::App;
use crate::error::HubApiError;
use crate::routes::plugins::head_of;

/// The window between the answer and the commit. Caveat: a guess above the kill latency (the
/// runner's 50 ms poll plus a `docker kill`); a host slower than five seconds to kill lets the
/// commit land first, and the control then fails to fail, which the row reports as red.
const COMMIT_DELAY: Duration = Duration::from_secs(5);

/// The answer the store would give `write`, as `(seq, response, epoch)`, with the commit deferred.
pub(crate) async fn answer_first(
    app: Arc<App>,
    write: BatchWrite,
) -> Result<(u64, String, u64), HubApiError> {
    let (epoch, head) = head_of(app.store().await?, &write.ws).await?;
    let applied = (write.batch.upserts.len() + write.batch.deletes.len()) as u64;
    let seq = head + 1;
    tokio::spawn(async move {
        tokio::time::sleep(COMMIT_DELAY).await;
        let error = match app.store().await {
            Ok(store) => store.apply_batch(&write).await.err().map(|e| e.to_string()),
            Err(e) => Some(e.code().into_owned()),
        };
        if let Some(error) = error {
            app.log(&serde_json::json!({ "event": "early-ack", "error": error }));
        }
    });
    Ok((seq, graph_contract::hub::answer_json(seq, applied), epoch))
}
