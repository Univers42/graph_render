//! This process's resident memory, from `/proc/self`: the high-water mark and its reset.
//!
//! A copy of `crates/graph-cli/src/bench/cap_probe/rss.rs`: graph-hub's tests cannot depend on
//! graph-cli, and three functions do not earn a shared crate.

/// Starts a new high-water mark at the current resident size (Linux ≥ 4.0). `false` where the file
/// is missing or not writable, and the mark then keeps the lifetime peak.
pub fn reset_peak() -> bool {
    std::fs::write("/proc/self/clear_refs", "5").is_ok()
}

/// A `kB` field of `/proc/self/status`, e.g. `VmRSS` or `VmHWM`; `None` off Linux.
pub fn status_kib(field: &str) -> Option<u64> {
    parse_status_kib(&std::fs::read_to_string("/proc/self/status").ok()?, field)
}

/// The `field` line of a `/proc/<pid>/status` text, in KiB: `VmHWM:\t  1792 kB` → 1792.
pub fn parse_status_kib(status: &str, field: &str) -> Option<u64> {
    status.lines().find_map(|line| {
        let value = line.strip_prefix(field)?.strip_prefix(':')?;
        value.trim().strip_suffix("kB")?.trim().parse().ok()
    })
}

/// A fresh high-water mark, in bytes: the case's baseline. Panics where it cannot be reset, since
/// a peak measured from an unknown baseline is not a measurement.
pub fn baseline() -> u64 {
    assert!(reset_peak(), "/proc/self/clear_refs is not writable");
    peak()
}

/// The high-water mark since [`baseline`], in bytes.
pub fn peak() -> u64 {
    status_kib("VmHWM").expect("VmHWM in /proc/self/status") * 1024
}
