# syntax=docker/dockerfile:1
#
# The profilers, on top of the graph-motor toolchain image (docker/rust.Dockerfile): valgrind
# (callgrind for exact instruction counts per function, cachegrind for cache misses, dhat for
# heap churn) and hyperfine for wall-clock repeats. Debian's own packages, so the versions are
# trixie's. Build ge-rust first, then
#
#   docker build -f docker/profile.Dockerfile -t ge-profile .
#   scripts/orch/profile.sh 100000          # see its header
#
# perf is not here: on this host perf_event_paranoid is 4 and the Docker daemon is rootless, so
# perf_event_open is refused inside any container. callgrind's counts are exact and noise-free,
# which is what a before/after comparison needs; wall time comes from `graph-cli tick` and
# hyperfine.
#
# Behind a TLS-intercepting proxy, pass its CA as the same optional secret ge-rust takes.

ARG BASE=ge-rust
FROM ${BASE}

RUN --mount=type=secret,id=extra_ca,required=false,mode=0444 \
    set -eu; \
    src=/etc/apt/sources.list.d/debian.sources; \
    if [ -s /run/secrets/extra_ca ]; then \
      sed -i 's#http://#https://#' "$src"; \
      echo 'Acquire::https::CAInfo "/run/secrets/extra_ca";' > /etc/apt/apt.conf.d/99extra-ca; \
    fi; \
    apt-get update; \
    apt-get install -y --no-install-recommends valgrind hyperfine; \
    sed -i 's#https://#http://#' "$src"; \
    rm -rf /var/lib/apt/lists/* /etc/apt/apt.conf.d/99extra-ca
