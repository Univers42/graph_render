# syntax=docker/dockerfile:1
#
# Differential oracle ONLY: pinned Graphviz 16.1.0 built from the release tarball, for
# checking the native twopi/circo/neato/fdp/sfdp/osage/patchwork ports. Never shipped and
# never a graph-core dependency (rule 0.2: nothing on the host).
#
# Ponytail: Graphviz is EPL-1.0 and is an oracle plus an algorithm reference only — it is
# never linked, vendored, or translated line by line into crates/. The image carries no
# network at run time and no compiler.
#
#   docker build --build-context gv="$GM_SCRATCH/refs/graphviz-16.1.0" \
#     -f docker/graphviz-oracle.Dockerfile -t ge-graphviz-oracle .
#   docker run --rm --user 0:0 -v $PWD:/w -w /w ge-graphviz-oracle \
#       python3 harness/oracle-graphviz.py target/spectral-fixtures twopi target/gv-twopi
#
# The `gv` named context is the directory holding graphviz-16.1.0.tar.gz (fetch-refs.sh).
#
# `libgts-dev` is here for `-Goverlap=prism`: `remove_overlap`'s real body is compiled only
# under `HAVE_GTS && SFDP` (lib/neatogen/overlap.c:17), so without GTS every `-Goverlap` value
# runs the same stub and prints "not built with triangulation library" (overlap.c:585-610) —
# measured byte-identical for false/true/scale/prism/vor. With GTS the stub is gone and prism
# is a real algorithm, which is what the overlap differential needs.

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
# build would succeed with prism still stubbed. The assertion below is what makes it loud.
RUN cd /opt/graphviz-src/graphviz-16.1.0 \
 && ./configure --prefix=/opt/graphviz --with-gts=yes > /tmp/configure.out 2>&1 \
 && grep -q '^ *gts: *Yes' /tmp/configure.out \
 && make -j"$(nproc)" \
 && make install \
 && rm -rf /opt/graphviz-src

ENV PATH=/opt/graphviz/bin:$PATH

RUN for e in twopi circo neato fdp sfdp osage patchwork dot; do "$e" -V 2>&1; done
