//! How many compute slots the memory ceiling holds (condition 3):
//! `workers = min(cores, (memory.max - base) / per_slot_bytes(max_body))`, and 0 refuses the start.

/// The body `GRAPH_MAX_BODY` defaults to, and the body every ingest peak below was measured at:
/// 64 MiB. `docs/measurements/service-caps.md` "Memory per slot" has the measurements.
pub const BODY_BYTES: u64 = 67_108_864;

/// The run peak one slot is charged, whatever the body: 3,343,908,864 B, the 3189 MiB peak of
/// `post.style.orthogonal` over `layout.circular.radial` at n 1048576, m 4194304. It is charged in
/// full at every body, including a small one no body can reach that graph through.
/// Caveat: one machine under load, and an over-estimate: it sits at a size the 64 MiB body cannot
/// carry, and the ingest and run peaks are counted as if they overlapped.
pub const RUN_PEAK_BYTES: u64 = 3_343_908_864;

/// The contract ingest heap peak over a [`BODY_BYTES`] body: 1,224,659,341 B, 18.25x the body. It
/// is the larger of the two ingest peaks (studio at 141,099,952 B), so it binds, and it is the term
/// that moves with `GRAPH_MAX_BODY`: [`per_slot_bytes`] charges this ratio times whatever body the
/// operator sets.
/// Caveat: one machine under load, a linear fit read off a single body size, and an over-estimate —
/// the ingest and run peaks are counted as if they overlapped, and no body other than 64 MiB was
/// measured end to end. An explicit `GRAPH_WORKERS` overrides the whole budget.
pub const INGEST_PEAK_BYTES: u64 = 1_224_659_341;

/// The memory one slot is budgeted at the default [`BODY_BYTES`] body: that body, plus the contract
/// ingest peak, plus the run peak. Written out, because `scripts/service-limits.sh` and
/// `scripts/service-max-body.sh` read this constant and the three terms above it with `grep`, and the
/// test below holds the two in step. [`per_slot_bytes`] is the figure for any other body.
pub const PER_SLOT_BYTES: u64 = 4_635_677_069;

/// The server's idle footprint, kept out of the slots: 11 MiB, the largest of three readings of
/// the image's `memory.current` taken right after `listening` (11,534,336 / 10,940,416 /
/// 8,011,776 B, `docs/measurements/service-caps.md` "Base").
/// Caveat: one sample's high-water mark, and idle. The three readings of a server with no request
/// in flight differ by 3.5 MiB, `memory.current` counts page cache and socket buffers the process
/// never held as RSS, and nothing here says what a busy slot's allocator arenas add on top — which
/// is what [`PER_SLOT_BYTES`] carries instead. Understating it grants one slot too many when
/// `memory.max` sits within it of a whole number of slots.
pub const BASE_BYTES: u64 = 11_534_336;

/// Where cgroup v2 publishes the container's memory ceiling.
const CGROUP_MEMORY_MAX: &str = "/sys/fs/cgroup/memory.max";

/// The memory one slot is budgeted for a `GRAPH_MAX_BODY` of `max_body`: the run peak, the body
/// itself, and the contract ingest peak scaled to it at the [`INGEST_PEAK_BYTES`] to [`BODY_BYTES`]
/// ratio. Saturating, so a body past the range `read_limits` accepts cannot wrap a slot figure
/// around to a small one.
pub fn per_slot_bytes(max_body: u64) -> u64 {
    let scaled = max_body
        .saturating_mul(INGEST_PEAK_BYTES)
        .div_ceil(BODY_BYTES);
    RUN_PEAK_BYTES
        .saturating_add(max_body)
        .saturating_add(scaled)
}

/// The default worker count for a `GRAPH_MAX_BODY` of `max_body`; 0 when `memory_max` is under one
/// slot of that body, which `read_workers` refuses as the start.
pub fn default_workers(cores: usize, max_body: u64, memory_max: Option<u64>) -> usize {
    let per_slot = per_slot_bytes(max_body);
    let by_memory = memory_max.map_or(usize::MAX, |bytes| {
        let slots = bytes.saturating_sub(BASE_BYTES) / per_slot;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_body_reproduces_the_measured_slot() {
        // PER_SLOT_BYTES is written out so the gate scripts can grep it; this holds it to the three
        // terms it is made of, so a term that moves cannot leave the figure behind.
        assert_eq!(
            RUN_PEAK_BYTES + BODY_BYTES + INGEST_PEAK_BYTES,
            PER_SLOT_BYTES
        );
        assert_eq!(per_slot_bytes(BODY_BYTES), PER_SLOT_BYTES);
    }

    #[test]
    fn the_ingest_term_scales_with_the_body_and_rounds_up() {
        let twice = per_slot_bytes(2 * BODY_BYTES);
        assert_eq!(
            twice - per_slot_bytes(BODY_BYTES),
            BODY_BYTES + INGEST_PEAK_BYTES,
            "one more 64 MiB of body costs one more body and one more ingest peak"
        );
        assert_eq!(
            per_slot_bytes(1 << 30),
            RUN_PEAK_BYTES + (1 << 30) + 16 * INGEST_PEAK_BYTES,
            "1 GiB is sixteen 64 MiB bodies"
        );
        assert!(
            per_slot_bytes(u64::MAX) > PER_SLOT_BYTES,
            "saturating, not wrapped"
        );
    }

    #[test]
    fn workers_follow_the_body_the_container_is_given() {
        let at_default = default_workers(8, BODY_BYTES, Some(8 << 30));
        assert_eq!(at_default, 1, "8 GiB holds one slot at the default body");
        assert_eq!(
            default_workers(8, 1 << 30, Some(8 << 30)),
            0,
            "8 GiB holds no slot at the 1 GiB body"
        );
        assert_eq!(
            default_workers(8, BODY_BYTES, None),
            8,
            "no ceiling is the core count"
        );
    }
}
