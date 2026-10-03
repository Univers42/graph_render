//! The `±inf` build probe, split from `../tests.rs` for the house 300-line cap. A build on
//! a non-finite coordinate did not return before the refusal, so the assertions run in a
//! child process this module caps: the suite can fail the row, never hang on it.

use super::super::Quadtree;
use super::{PROBE_NAME, visited_points};

/// Every non-finite point set the build must refuse and return on. `±inf` is the case the
/// review named: `bounds_of` filtered only NaN, so `cover` grew until the bounds went NaN
/// and `insert_leaf` split forever, pushing an internal node every iteration.
fn non_finite_cases() -> Vec<(&'static str, Vec<f64>, Vec<f64>)> {
    vec![
        ("+inf x", vec![f64::INFINITY, 0.0], vec![0.0, 1.0]),
        ("-inf x", vec![f64::NEG_INFINITY, 0.0], vec![0.0, 1.0]),
        ("+inf y", vec![0.0, 1.0], vec![f64::INFINITY, 2.0]),
        ("-inf y", vec![0.0, 1.0], vec![2.0, f64::NEG_INFINITY]),
        (
            "all non-finite",
            vec![f64::INFINITY],
            vec![f64::NEG_INFINITY],
        ),
    ]
}

/// Every case must come back with a finite square, the refused point in no leaf, and
/// `order` still a permutation: the tree is smaller, not broken.
fn assert_refused(name: &str, xs: &[f64], ys: &[f64]) {
    let mut tree = Quadtree::default();
    tree.build(xs, ys);
    let finite = (0..xs.len())
        .filter(|&i| xs[i].is_finite() && ys[i].is_finite())
        .count();
    if finite > 0 {
        let b = tree.root_bounds;
        assert!(
            b.x0.is_finite() && b.y0.is_finite(),
            "{name}: finite bounds"
        );
    }
    assert_eq!(
        tree.order().len(),
        xs.len(),
        "{name}: order is still a permutation"
    );
    assert_eq!(
        visited_points(&tree).len(),
        finite,
        "{name}: {finite} finite point(s), one per leaf"
    );
    if finite == 0 {
        assert!(tree.root.is_none(), "{name}: no point is finite");
        assert_eq!(visited_points(&tree), Vec::<u32>::new());
        return;
    }
    // The refused points are in no leaf: every leaf's chain is finite, and the finite
    // indices are exactly the ones visited.
    let expected: Vec<u32> = (0..xs.len() as u32)
        .filter(|&i| xs[i as usize].is_finite() && ys[i as usize].is_finite())
        .collect();
    let mut seen = visited_points(&tree);
    seen.sort_unstable();
    assert_eq!(
        seen, expected,
        "{name}: only the finite points are in leaves"
    );
    // And they follow every leaf in `order`, ascending, as a NaN point already did.
    let refused: Vec<u32> = (0..xs.len() as u32)
        .filter(|&i| !(xs[i as usize].is_finite() && ys[i as usize].is_finite()))
        .collect();
    assert_eq!(
        tree.order()[expected.len()..].to_vec(),
        refused,
        "{name}: the refused points follow the leaves, ascending"
    );
}

/// The probe's own body. `#[ignore]`d because on the unrepaired tree it does not return;
/// [`run`] executes it in a capped child, so this suite can never hang on it.
#[test]
#[ignore = "run in a child by tests::a_non_finite_coordinate_is_refused_and_the_build_returns"]
fn a_non_finite_coordinate_makes_the_build_return() {
    for (name, xs, ys) in non_finite_cases() {
        assert_refused(name, &xs, &ys);
    }
}

/// Runs [`a_non_finite_coordinate_makes_the_build_return`] in a child process under an
/// address-space cap and a 60 s deadline, and returns its exit status and stderr.
///
/// Ponytail: `ulimit -v` caps *reserved* address space, not resident bytes, so the child
/// aborts somewhere inside the 4 GB rather than at a reproducible point, and a wasm32
/// build reserves differently. It bounds the damage; it does not reproduce the hang, and
/// on a host where `/bin/sh` is absent the row fails rather than skips. Escape hatch:
/// none — a non-terminating build has to fail loudly somewhere.
pub(super) fn run() -> (std::process::ExitStatus, String) {
    let exe = std::env::current_exe().expect("the test binary's own path");
    let script = format!("ulimit -v 4000000; exec \"$0\" --exact {PROBE_NAME} --ignored");
    let mut child = std::process::Command::new("/bin/sh")
        .arg("-c")
        .arg(script)
        .arg(exe)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("a shell to cap the probe's address space");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    let status = loop {
        match child.try_wait().expect("the probe child's status") {
            Some(status) => break status,
            None if std::time::Instant::now() > deadline => {
                child.kill().expect("the probe child's kill");
                panic!("{PROBE_NAME} did not return within 60 s");
            }
            None => std::thread::sleep(std::time::Duration::from_millis(50)),
        }
    };
    let out = child.wait_with_output().expect("the probe child's output");
    let mut said = String::from_utf8_lossy(&out.stdout).into_owned();
    said.push_str(&String::from_utf8_lossy(&out.stderr));
    (status, said)
}
