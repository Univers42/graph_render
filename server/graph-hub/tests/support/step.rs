//! The test half of `scripts/orch/hub-run.sh run`'s step handshake.
//!
//! The test runs as root inside a `scripts/orch/gr` container and cannot reach docker, so it asks
//! the runner on the host for a container action: it writes one verb to
//! `target/hub-steps/<step>.req` (through a rename, so the runner never reads half a verb) and
//! waits for `<step>.ack`, which holds the action's exit status. It works because `gr` bind-mounts
//! the repository read-write at `/w`, so both sides see one `target/`.

use std::path::PathBuf;
use std::time::{Duration, Instant};

/// How long one action may take. Caveat: a guess above a cold container start plus its health
/// wait (`hub-run.sh`'s own 60 s); a slower host fails here with the step named, never hangs.
const ACK_TIMEOUT: Duration = Duration::from_secs(120);

/// The poll period, the runner's own 50 ms.
const POLL: Duration = Duration::from_millis(50);

/// `target/hub-steps`, which the runner made on the host before the test started.
pub fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/hub-steps")
}

/// Ask the runner to `verb` (kill, stop, start or restart) the hub container, under `step`.
pub fn request(step: &str, verb: &str) {
    let ack = dir().join(format!("{step}.ack"));
    let _ = std::fs::remove_file(&ack);
    let tmp = dir().join(format!("{step}.req.tmp"));
    std::fs::write(&tmp, format!("{verb}\n")).expect("the step request");
    std::fs::rename(&tmp, dir().join(format!("{step}.req"))).expect("the step request's rename");
}

/// The exit status the runner wrote for `step`, polling until [`ACK_TIMEOUT`].
pub async fn await_ack(step: &str) -> i32 {
    let ack = dir().join(format!("{step}.ack"));
    let start = Instant::now();
    loop {
        if let Ok(text) = std::fs::read_to_string(&ack) {
            return text.trim().parse().expect("a numeric exit status");
        }
        assert!(
            start.elapsed() < ACK_TIMEOUT,
            "no ack for step {step}: is the test running under hub-run.sh run?"
        );
        tokio::time::sleep(POLL).await;
    }
}

/// [`request`], then [`await_ack`], asserting the action succeeded.
pub async fn act(step: &str, verb: &str) {
    request(step, verb);
    let status = await_ack(step).await;
    assert_eq!(status, 0, "step {step} ({verb}) exited {status}");
}

/// Write `seqs`, one per line, to `target/hub-steps/<name>`, for a later test process.
pub fn record(name: &str, seqs: &[u64]) {
    let text: String = seqs.iter().map(|seq| format!("{seq}\n")).collect();
    std::fs::write(dir().join(name), text).expect("the record file");
}

/// The seqs [`record`] wrote under `name`.
pub fn recorded(name: &str) -> Vec<u64> {
    let path = dir().join(name);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{}: {error}; run the writer case first", path.display()));
    text.lines()
        .map(|line| line.parse().expect("a seq per line"))
        .collect()
}
