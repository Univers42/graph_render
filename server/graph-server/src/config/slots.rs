//! How many compute slots the memory ceiling holds (condition 3):
//! `workers = min(cores, floor((memory.max - base) / per_slot))`, and 0 refuses the start.

/// The memory one slot is budgeted: the 64 MiB body, plus the larger of the two ingest heap
/// peaks — studio at 141,099,952 B and contract at 1,224,659,341 B, so contract binds — plus
/// the run peak at the largest cap of 3189 MiB (`post.style.orthogonal` over
/// `layout.circular.radial` at n 1048576, m 4194304). `docs/measurements/service-caps.md`
/// "Memory per slot" has the measurements.
/// Caveat: one machine under load, and an over-estimate: the run peak sits at a size the 64 MiB
/// body cannot carry, and the ingest and run peaks are counted as if they overlapped. An
/// explicit `GRAPH_WORKERS` overrides it.
pub const PER_SLOT_BYTES: u64 = 4_635_677_069;

/// The server's idle footprint, kept out of the slots.
/// Caveat: 0, not measured: the idle RSS is a few MiB against a 4.3 GiB slot, so the error only
/// bites when memory.max sits within that much of a whole number of slots, where it grants one
/// slot too many.
pub const BASE_BYTES: u64 = 0;

/// Where cgroup v2 publishes the container's memory ceiling.
const CGROUP_MEMORY_MAX: &str = "/sys/fs/cgroup/memory.max";

/// The default worker count; 0 when `memory_max` is under one slot.
pub fn default_workers(cores: usize, memory_max: Option<u64>) -> usize {
    let by_memory = memory_max.map_or(usize::MAX, |bytes| {
        let slots = bytes.saturating_sub(BASE_BYTES) / PER_SLOT_BYTES;
        usize::try_from(slots).unwrap_or(usize::MAX)
    });
    cores.min(by_memory)
}

/// The cgroup's memory ceiling, or `None` where there is none (`max`) or no cgroup v2.
pub fn cgroup_memory_max() -> Option<u64> {
    std::fs::read_to_string(CGROUP_MEMORY_MAX)
        .ok()?
        .trim()
        .parse()
        .ok()
}
