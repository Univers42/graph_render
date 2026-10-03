# syntax=docker/dockerfile:1
#
# Differential oracle ONLY: pinned Graphviz 16.1.0 built from the release tarball, for
# checking the native twopi/circo/neato/fdp/sfdp/osage/patchwork ports. Never shipped and
# never a graph-core dependency (rule 0.2: nothing on the host).
#
# Ponytail: Graphviz is EPL-1.0 and is an oracle plus an algorithm reference only — it is
# never linked, vendored, or translated line by line into crates/. The image carries no network
# at run time. It does carry a compiler: the single-stage `build-essential` below is kept, and
# that is deliberate, because the exact-coordinate reader `harness/scigraphs-conformance/gv_exact.c`
# is compiled at run time against the headers installed at `/opt/graphviz/include/graphviz`, so
# the reference arm can read coordinates without going through `-Tplain`'s `%.5g` text.
#
#   docker build --build-context gv="$GM_SCRATCH/refs/graphviz-16.1.0" \
#     -f docker/graphviz-oracle.Dockerfile -t ge-graphviz-oracle .
#   docker run --rm --user 0:0 -v $PWD:/w -w /w ge-graphviz-oracle \
#       python3 harness/oracle-graphviz.py target/spectral-fixtures twopi target/gv-twopi
#
# The `gv` named context is the directory holding graphviz-16.1.0.tar.gz (fetch-refs.sh).

FROM debian:trixie-slim

ENV DEBIAN_FRONTEND=noninteractive

RUN apt-get update \
 && apt-get install -y --no-install-recommends \
      python3 ca-certificates build-essential pkg-config bison flex \
      libexpat1-dev zlib1g-dev libgd-dev libfreetype6-dev libfontconfig1-dev \
      libltdl3-dev \
 && rm -rf /var/lib/apt/lists/*

ARG GRAPHVIZ_SHA256=beea483ab130f456c1c3905f4f2e40778a9c493c3d73ae8012367d552d71ca84
COPY --from=gv graphviz-16.1.0.tar.gz /tmp/graphviz-16.1.0.tar.gz
RUN echo "${GRAPHVIZ_SHA256}  /tmp/graphviz-16.1.0.tar.gz" | sha256sum -c - \
 && mkdir -p /opt/graphviz-src \
 && tar -xzf /tmp/graphviz-16.1.0.tar.gz -C /opt/graphviz-src \
 && rm /tmp/graphviz-16.1.0.tar.gz

RUN cd /opt/graphviz-src/graphviz-16.1.0 \
 && ./configure --prefix=/opt/graphviz \
 && make -j"$(nproc)" \
 && make install \
 && rm -rf /opt/graphviz-src

ENV PATH=/opt/graphviz/bin:$PATH

RUN for e in twopi circo neato fdp sfdp osage patchwork dot; do "$e" -V 2>&1; done
