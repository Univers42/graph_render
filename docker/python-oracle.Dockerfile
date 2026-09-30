# syntax=docker/dockerfile:1
#
# Differential oracle ONLY: pinned scipy/networkx for checking the Rust FA2 and LOBPCG
# ports. Never shipped and never a graph-core dependency (rule 0.2: nothing on the host).
#
# Ponytail: checks against a single upstream version (scipy 1.16.2, networkx 3.6) with
# OpenBLAS numerics, so agreement is to a tolerance, not bitwise across BLAS builds.
#
#   docker build --build-context nx="$GM_SCRATCH/refs/networkx-3.6" \
#     -f docker/python-oracle.Dockerfile -t ge-python-oracle .
#   docker run --rm ge-python-oracle python3 -c "import networkx,numpy,scipy;print(networkx.__version__,numpy.__version__,scipy.__version__)"
#
# The `nx` named context is the directory holding networkx-3.6.tar.gz (fetch-refs.sh).

FROM debian:trixie-slim

ENV OPENBLAS_NUM_THREADS=1 OMP_NUM_THREADS=1 MKL_NUM_THREADS=1 \
    PYTHONDONTWRITEBYTECODE=1 PIP_DISABLE_PIP_VERSION_CHECK=1

RUN apt-get update \
 && apt-get install -y --no-install-recommends python3 python3-venv ca-certificates \
 && rm -rf /var/lib/apt/lists/*

COPY docker/python-oracle.requirements.txt /tmp/requirements.txt
RUN python3 -m venv /opt/oracle-venv \
 && /opt/oracle-venv/bin/pip install --require-hashes --only-binary=:all: --no-deps \
      -r /tmp/requirements.txt

ARG NETWORKX_SHA256=285276002ad1f7f7da0f7b42f004bcba70d381e936559166363707fdad3d72ad
COPY --from=nx networkx-3.6.tar.gz /tmp/networkx-3.6.tar.gz
RUN echo "${NETWORKX_SHA256}  /tmp/networkx-3.6.tar.gz" | sha256sum -c - \
 && mkdir -p /opt/oracle-src \
 && tar -xzf /tmp/networkx-3.6.tar.gz -C /opt/oracle-src \
 && rm /tmp/networkx-3.6.tar.gz

ENV PATH=/opt/oracle-venv/bin:$PATH PYTHONPATH=/opt/oracle-src/networkx-3.6

RUN python3 -c "import scipy,numpy,networkx; assert scipy.__version__=='1.16.2' and networkx.__version__=='3.6'; print('numpy', numpy.__version__)"
