# syntax=docker/dockerfile:1
#
# Differential oracle ONLY: pinned scipy/networkx/igraph for checking the Rust FA2, LOBPCG
# and igraph-family layout ports. Never shipped and never a graph-core dependency (rule 0.2: nothing on the host).
#
# Ponytail: checks against a single upstream version (scipy 1.16.2, networkx 3.6, igraph
# 0.11.9) with OpenBLAS numerics, so agreement is to a tolerance, not bitwise across BLAS builds.
#
#   docker build --build-context nx=/goinfre/dlesieur/refs/networkx-3.6 \
#     --build-context ig=/goinfre/dlesieur/refs/igraph-0.11.9 \
#     -f docker/python-oracle.Dockerfile -t ge-python-oracle .
#   docker run --rm ge-python-oracle python3 -c "import networkx,numpy,scipy;print(networkx.__version__,numpy.__version__,scipy.__version__)"
#
# The `nx` and `ig` named contexts are the directories holding networkx-3.6.tar.gz and
# igraph-0.11.9.tar.gz (fetch-refs.sh). igraph (the PyPI name of python-igraph since
# 0.10) is built here from that digest-checked sdist, C core included, in a throwaway stage;
# only the wheel reaches the oracle image.

FROM debian:trixie-slim AS igraph-build

ENV PYTHONDONTWRITEBYTECODE=1 PIP_DISABLE_PIP_VERSION_CHECK=1
RUN apt-get update \
 && apt-get install -y --no-install-recommends python3 python3-venv python3-dev \
      build-essential cmake flex bison ca-certificates \
 && rm -rf /var/lib/apt/lists/*
COPY docker/python-oracle.build-requirements.txt /tmp/build-requirements.txt
RUN python3 -m venv /opt/build-venv \
 && /opt/build-venv/bin/pip install --require-hashes --only-binary=:all: --no-deps \
      -r /tmp/build-requirements.txt
ARG IGRAPH_SHA256=c57ce44873abcfcfd1d61d7d261e416d352186958e7b5d299cf244efa6757816
COPY --from=ig igraph-0.11.9.tar.gz /tmp/igraph-0.11.9.tar.gz
RUN echo "${IGRAPH_SHA256}  /tmp/igraph-0.11.9.tar.gz" | sha256sum -c - \
 && mkdir /tmp/igraph-src && tar -xzf /tmp/igraph-0.11.9.tar.gz -C /tmp/igraph-src \
 && cd /tmp/igraph-src/igraph-0.11.9 \
 && /opt/build-venv/bin/pip wheel --no-build-isolation --no-deps -w /wheels .

FROM debian:trixie-slim

ENV OPENBLAS_NUM_THREADS=1 OMP_NUM_THREADS=1 MKL_NUM_THREADS=1 \
    PYTHONDONTWRITEBYTECODE=1 PIP_DISABLE_PIP_VERSION_CHECK=1

RUN apt-get update \
 && apt-get install -y --no-install-recommends python3 python3-venv ca-certificates libgomp1 \
 && rm -rf /var/lib/apt/lists/*

COPY docker/python-oracle.requirements.txt /tmp/requirements.txt
RUN python3 -m venv /opt/oracle-venv \
 && /opt/oracle-venv/bin/pip install --require-hashes --only-binary=:all: --no-deps \
      -r /tmp/requirements.txt

COPY --from=igraph-build /wheels/ /tmp/wheels/
RUN /opt/oracle-venv/bin/pip install --no-index --no-deps /tmp/wheels/igraph-0.11.9-*.whl \
 && rm -rf /tmp/wheels

ARG NETWORKX_SHA256=285276002ad1f7f7da0f7b42f004bcba70d381e936559166363707fdad3d72ad
COPY --from=nx networkx-3.6.tar.gz /tmp/networkx-3.6.tar.gz
RUN echo "${NETWORKX_SHA256}  /tmp/networkx-3.6.tar.gz" | sha256sum -c - \
 && mkdir -p /opt/oracle-src \
 && tar -xzf /tmp/networkx-3.6.tar.gz -C /opt/oracle-src \
 && rm /tmp/networkx-3.6.tar.gz

ENV PATH=/opt/oracle-venv/bin:$PATH PYTHONPATH=/opt/oracle-src/networkx-3.6

RUN python3 -c "import scipy,numpy,networkx,igraph; assert scipy.__version__=='1.16.2' and networkx.__version__=='3.6' and igraph.__version__=='0.11.9'; print('numpy', numpy.__version__)"
