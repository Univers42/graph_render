//! The container half of Task 10: `HUB_MEM_WRITERS` writes of the worst body shape at `MAX_BODY`,
//! all in the hub at once. `scripts/orch/hub-mem.sh` starts the hub capped at 1 GiB with
//! `GM_HUB_HOLD_BODIES` equal to the writer count, runs this case, and reads the container's peak.
//! The case only proves every write was answered: a hub the kernel killed under the cap ends each
//! write on a transport error, and the script reads the kill from the container's state.

use crate::PLUGIN;
use crate::bodies;
use crate::support::db;
use crate::support::wire::Remote;
use graph_contract::hub::Limits;
use std::sync::Arc;
use std::time::Duration;

/// How long one write may take. Caveat: a guess, measured on nothing slower than the run in
/// `docs/measurements/hub-memory.md`; the barrier parks every write until the last one has read
/// its body, so a slow host needs a larger value, and a hub killed mid-run ends the writes on a
/// transport error well before it.
const WRITE_TIMEOUT: Duration = Duration::from_secs(120);

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "run by scripts/orch/hub-mem.sh against a hub it started"]
async fn peak_rss_at_every_cap_fits_one_gib() {
    let writers: usize = std::env::var("HUB_MEM_WRITERS")
        .expect("HUB_MEM_WRITERS: run through scripts/orch/hub-mem.sh")
        .parse()
        .expect("a writer count");
    db::migrated().await;
    let limits = Limits::DEFAULT;
    let body = bodies::body("zeros", limits.max_body as usize, limits.max_batch as usize);
    let body: Arc<str> = Arc::from(body);
    let remote = Arc::new(Remote::with_timeout(WRITE_TIMEOUT));
    // One workspace per writer, so the store's per-workspace order cannot serialise the parses.
    for w in 0..writers {
        remote.ready(&workspace(w), PLUGIN).await;
    }
    let posts: Vec<_> = (0..writers)
        .map(|w| {
            let (remote, body) = (Arc::clone(&remote), Arc::clone(&body));
            tokio::spawn(async move {
                let ws = workspace(w);
                remote.post_batch(&ws, PLUGIN, &body, &ws).await
            })
        })
        .collect();
    for (w, post) in posts.into_iter().enumerate() {
        let reply = post.await.expect("a writer task");
        let reply = reply.unwrap_or_else(|error| panic!("writer {w}: {error}"));
        assert_eq!(reply.code(), 200, "writer {w}: {}", reply.body());
    }
    println!("HUB_MEM container writers={writers} body={}", body.len());
}

fn workspace(w: usize) -> String {
    format!("mem-{w}")
}
