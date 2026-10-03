//! What `build_wasm` must prove about a build it did not watch, and the one thing it must
//! never do to the artifact while another process holds its path — the two halves of RG-51's
//! `$CARGO` row side by side, because satisfying the first by breaking the second is exactly
//! what made `--workspace` red: an artifact deleted before every rebuild is absent for as
//! long as that rebuild takes, and a gate already reading it refuses with exit 2.

mod common;

use common::stdout;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

fn gates_dir(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!("gm-cli-wasm-build-{label}-{}", std::process::id()))
}

/// The artifact `build_wasm` returns, spelled the way `runner.rs` spells it: the workspace
/// root's `target/`, which `CARGO_MANIFEST_DIR/../..` is.
fn artifact() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("target")
        .join("wasm32-unknown-unknown")
        .join("release")
        .join("graph_wasm.wasm")
}

/// RG-51's own repro: `$CARGO` is trusted only as far as its exit status, so a wrapper that
/// exits 0 having built nothing must be refused (exit 2) rather than leave the gate to hash
/// whatever `graph_wasm.wasm` already was. `/bin/true` is that wrapper.
#[test]
fn a_cargo_that_exits_zero_without_naming_the_artifact_is_refused() {
    let run = common::command(&gates_dir("no-op"))
        .args(["hashgate", "--seeds", "4"])
        .env("CARGO", "/bin/true")
        .output()
        .expect("graph-cli runs");
    assert_eq!(run.status.code(), Some(2), "{}", stdout(&run));
    let stderr = String::from_utf8_lossy(&run.stderr);
    assert!(
        stderr.contains("exited 0 without naming") && stderr.contains("graph_wasm.wasm"),
        "{stderr}"
    );
}

/// The window a delete-before-build opens, watched: the shared artifact has to be there for
/// every instant of a gate run. `build_wasm` used to delete it before building so a wrapper
/// that built nothing would leave nothing behind — which made it absent for the whole of
/// every rebuild, and any second process already holding the path then read "no such file"
/// and refused with exit 2. `absent` counts the sampled instants the path was gone.
#[test]
fn the_shared_artifact_is_present_for_every_instant_of_a_gate_run() {
    let wasm = artifact();
    let warm = common::graph_cli(&gates_dir("watch"), &["hashgate", "--seeds", "4"], None);
    assert_eq!(warm.status.code(), Some(0), "{}", stdout(&warm));
    assert!(wasm.is_file(), "the artifact to watch: {}", wasm.display());

    let watching = AtomicBool::new(true);
    let samples = AtomicUsize::new(0);
    let absent = AtomicUsize::new(0);
    let run = std::thread::scope(|scope| {
        scope.spawn(|| {
            while watching.load(Ordering::Relaxed) {
                samples.fetch_add(1, Ordering::Relaxed);
                if !wasm.is_file() {
                    absent.fetch_add(1, Ordering::Relaxed);
                }
                std::thread::sleep(Duration::from_millis(1));
            }
        });
        let run = common::graph_cli(&gates_dir("watch"), &["hashgate", "--seeds", "4"], None);
        watching.store(false, Ordering::Relaxed);
        run
    });
    assert_eq!(run.status.code(), Some(0), "{}", stdout(&run));
    assert!(
        samples.load(Ordering::Relaxed) > 50,
        "the watcher sampled nothing"
    );
    assert_eq!(
        absent.load(Ordering::Relaxed),
        0,
        "graph_wasm.wasm was absent while a gate was running"
    );
}
