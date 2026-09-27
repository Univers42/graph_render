//! Embeds the fingerprint of the tree graph-cli is built from as `GM_BUILD_FINGERPRINT`,
//! so a gate can refuse to record results under a tree its binary was not built from.

#[path = "src/fingerprint.rs"]
mod fingerprint;

use std::path::PathBuf;

fn main() {
    let manifest = std::env::var_os("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR");
    let root = PathBuf::from(manifest).join("..").join("..");
    for entry in fingerprint::FINGERPRINTED {
        println!("cargo::rerun-if-changed={}", root.join(entry).display());
    }
    match fingerprint::fingerprint_of(&root, &fingerprint::FINGERPRINTED) {
        Ok(digest) => println!("cargo::rustc-env=GM_BUILD_FINGERPRINT={digest}"),
        Err(err) => panic!("graph-cli build: {err}"),
    }
}
