//! The child runner's own three claims: a child's **status and raw stdout bytes** come back
//! together, a non-zero exit is `Ok` rather than `Err`, and the deadline kills.

use super::*;

#[test]
fn a_captured_child_gives_its_status_and_its_stdout_bytes() {
    let mut ok = Command::new("sh");
    ok.args(["-c", "printf 'a\\nb\\n'"]);
    let (status, stdout) = run_captured(&mut ok, Duration::from_secs(60)).expect("runs");
    assert!(status.success(), "exit 0");
    assert_eq!(String::from_utf8(stdout).expect("utf-8"), "a\nb\n");

    let mut failing = Command::new("sh");
    failing.args(["-c", "printf 'partial'; exit 3"]);
    let (status, stdout) = run_captured(&mut failing, Duration::from_secs(60)).expect("ran");
    assert_eq!(
        status.code(),
        Some(3),
        "a failed child is `Ok` with a status, not `Err`: the caller reads the bytes of a \
         build that failed, so an `Err` here would throw them away"
    );
    assert_eq!(String::from_utf8(stdout).expect("utf-8"), "partial");

    let mut absent = Command::new("gm-no-such-program");
    let err = run_captured(&mut absent, Duration::from_secs(60)).expect_err("never spawned");
    assert!(err.contains("gm-no-such-program"), "{err}");
}

/// The deadline this module's callers bound every child by, including the one that captures
/// bytes: a build whose output nobody reads must still be killed rather than waited on.
#[test]
fn a_captured_child_past_its_time_limit_is_killed_and_reported() {
    let mut slow = Command::new("sh");
    slow.args(["-c", "sleep 5"]);
    let started = Instant::now();
    let err = run_captured(&mut slow, Duration::from_millis(200)).expect_err("killed");
    assert!(err.contains("killed after 200ms"), "{err}");
    assert!(
        started.elapsed() < Duration::from_secs(4),
        "the kill must not wait the child's own five seconds"
    );
}

/// A stdout larger than a pipe buffer is the case the drain thread exists for: a child that
/// fills the pipe and blocks is the deadlock a plain `.output()` call would sit in forever.
#[test]
fn a_captured_child_whose_stdout_fills_the_pipe_is_not_stalled() {
    let mut loud = Command::new("sh");
    loud.args(["-c", "yes gm | head -c 400000"]);
    let (status, stdout) = run_captured(&mut loud, Duration::from_secs(60)).expect("runs");
    assert!(status.success());
    assert_eq!(stdout.len(), 400_000, "every byte, not the first pipe-full");
    assert!(stdout.starts_with(b"gm\ngm\n"));
}
