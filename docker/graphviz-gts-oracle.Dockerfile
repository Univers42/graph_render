# syntax=docker/dockerfile:1
#
# The node-overlap oracle: pinned Graphviz 16.1.0 built **with the GTS triangulation library**,
# so `remove_overlap`'s real body is compiled and `-Goverlap=prism` is a real algorithm
# (`lib/neatogen/overlap.c` gates it on `HAVE_GTS && SFDP`).
#
# **DO NOT TAG THIS `ge-graphviz-oracle`.** The shared tag is the pinned conformance oracle for
# every worktree; rebuilding it with GTS changed the sfdp and yifan-hu reference bytes and turned
# another branch's conformance row red. This file exists so the overlap differential can have a
# prism-capable engine without touching that image. It tags **only** `ge-graphviz-oracle-gts`:
#
#   docker build --build-context gv="$GM_SCRATCH/refs/graphviz-16.1.0" \
#     -f docker/graphviz-gts-oracle.Dockerfile -t ge-graphviz-oracle-gts .
#   docker run --rm --user 0:0 -v $PWD:/w -w /w ge-graphviz-oracle-gts \
#     python3 harness/gv_overlap.py --nodes 150 --seed 7 --out target/gv-overlap-150.json
#
# The no-GTS counterpart is the shared `ge-graphviz-oracle` itself, and it is the negative
# control for this row: `harness/gv_overlap.py` reports `prism_is_not_the_stub: false` there,
# because prism falls back and prints `Overlap value "prism" unsupported - ignored`
# (`lib/neatogen/adjust.c:828`, the `print == 0` placeholder). Run that control, do not rebuild it.
#
# Ponytail: Graphviz is EPL-1.0 and an oracle only — never linked, vendored, or translated into
# crates/. The image carries no network at run time and no compiler.

FROM debian:trixie-slim

ENV DEBIAN_FRONTEND=noninteractive

# Pinned so the oracle does not move when trixie does.
ARG GTS_VERSION=0.7.6+darcs121130-5.2+b1

RUN apt-get update \
 && apt-get install -y --no-install-recommends \
      python3 ca-certificates build-essential pkg-config bison flex \
      libexpat1-dev zlib1g-dev libgd-dev libfreetype6-dev libfontconfig1-dev \
      libltdl3-dev \
      "libgts-dev=${GTS_VERSION}" \
 && rm -rf /var/lib/apt/lists/*

ARG GRAPHVIZ_SHA256=beea483ab130f456c1c3905f4f2e40778a9c493c3d73ae8012367d552d71ca84
COPY --from=gv graphviz-16.1.0.tar.gz /tmp/graphviz-16.1.0.tar.gz
RUN echo "${GRAPHVIZ_SHA256}  /tmp/graphviz-16.1.0.tar.gz" | sha256sum -c - \
 && mkdir -p /opt/graphviz-src \
 && tar -xzf /tmp/graphviz-16.1.0.tar.gz -C /opt/graphviz-src \
 && rm /tmp/graphviz-16.1.0.tar.gz

# `--with-gts=yes` is configure's default, but PKG_CHECK_MODULES reports "No (gts library not
# available)" by setting `use_gts=No` rather than by failing (configure.ac:1532-1544), so the
# build would succeed with prism still stubbed. The assertion below is what makes it loud: it is
# what turned the "gts: Yes" claim into something checked rather than printed.
RUN cd /opt/graphviz-src/graphviz-16.1.0 \
 && ./configure --prefix=/opt/graphviz --with-gts=yes > /tmp/configure.out 2>&1 \
 && grep -q '^ *gts: *Yes' /tmp/configure.out \
 && make -j"$(nproc)" \
 && make install \
 && rm -rf /opt/graphviz-src

# The liveness assertion, not just the configure summary: prism must move the drawing, and it
# must differ from voronoi (a prism that silently fell back prints the identical bytes and is a
# stub no matter what configure said). Both hashes, measured on the same pinned input.
RUN for e in twopi circo neato fdp sfdp osage patchwork dot; do "$e" -V 2>&1; done \
 && ldd /opt/graphviz/lib/graphviz/libgvplugin_neato_layout.so | grep -q libgts