# graph-motor as a service: graph-server and the embed bundle (docs/deploy/service.md).
# Built by scripts/service.sh build, whose staging directory is the whole context: this file
# copies artifacts in and builds nothing, so no second rustup or node recipe exists. The binary
# was linked inside ge-rust, itself trixie, against the glibc this base carries: the digest is
# the trixie-slim ge-rust's first layer comes from (same RootFS layer a6dc7651..., 2026-10-03).
# Moving it is one edit here and a rebuild of ge-rust from the same digest.
FROM debian:trixie-slim@sha256:a99cfc517144bc59b1978475ec53b46ecabec7e43635402ee5b77cc54cd1b20a

COPY bin/graph-server /usr/local/bin/graph-server
COPY embed /srv/embed

ENV GRAPH_BIND=0.0.0.0 GRAPH_PORT=8080 GRAPH_EMBED_DIR=/srv/embed
# Numeric, so no RUN layer adds a passwd entry; the copied files stay root-owned and read-only
# to the process. The key file is mounted at run time, never baked in (scripts/service.sh run).
USER 10001:10001
EXPOSE 8080

# The base has no curl: the server probes its own /healthz (docs/contract/service-api.md, C11).
# Caveat: --timeout is the probe's own 2 s budget (server health.rs) plus 1 s for docker to start
# the exec; on a host loaded past that margin a live server reads as unhealthy, never the reverse.
HEALTHCHECK --interval=10s --timeout=3s --start-period=10s --start-interval=1s --retries=3 \
  CMD ["/usr/local/bin/graph-server", "healthcheck"]

ENTRYPOINT ["/usr/local/bin/graph-server"]
