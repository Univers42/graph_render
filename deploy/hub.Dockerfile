# The hub's runtime image. COPY-only, like deploy/service.Dockerfile:2-3: every cargo invocation
# lives in scripts/hub.sh through scripts/orch/gr, so no second rustup recipe exists.
FROM debian:trixie-slim@sha256:a99cfc517144bc59b1978475ec53b46ecabec7e43635402ee5b77cc54cd1b20a
COPY bin/graph-hub /usr/local/bin/graph-hub
ENV GRAPH_HUB_BIND=0.0.0.0 GRAPH_HUB_PORT=8080
USER 10001:10001
EXPOSE 8080
# No curl in the base: the binary probes its own /healthz, as graph-server's does.
HEALTHCHECK --interval=10s --timeout=3s --start-period=10s --retries=3 CMD ["/usr/local/bin/graph-hub", "healthcheck"]
ENTRYPOINT ["/usr/local/bin/graph-hub"]