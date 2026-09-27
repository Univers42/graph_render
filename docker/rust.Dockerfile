# syntax=docker/dockerfile:1
#
# The Rust toolchain for graph-motor, containerised. Rule 0.2: nothing is installed on
# the host, so this image *is* the toolchain. Rule 0.3: no prebuilt vendor language
# image, so it is a minimal Debian base plus exactly what we install, each pinned.
#
#   docker build -f docker/rust.Dockerfile -t ge-rust .
#   docker run --rm -v "$PWD:/w" ge-rust cargo test --workspace
#
# CARGO_HOME and RUSTUP_HOME live under /opt, not under the workdir: /w is a bind
# mount at run time, and a toolchain installed beneath it would be shadowed by the
# host checkout.
#
# Node is installed too, pinned to the release node:22-slim carries, because
# `graph-cli hashgate` runs its wasm32 arm under Node from inside this container.
#
# Behind a TLS-intercepting proxy, hand its CA bundle over as an optional secret:
#   docker build --secret id=extra_ca,src=/path/ca.crt -f docker/rust.Dockerfile -t ge-rust .
# Without the secret the build uses the system CA store; a secret never reaches a layer.

FROM debian:trixie-slim

ARG RUST_VERSION=1.98.1
ARG RUSTUP_VERSION=1.29.1
ARG RUSTUP_SHA256_AMD64=dda7234360b7f578ca8b0ddcb80145646fa61a67c1720a5abc7051b35c9fcb71
ARG RUSTUP_SHA256_ARM64=15f6e4ce9f583b929c996c91562bad6d4454f3281de858b02cdfdef615fac433
ARG NODE_VERSION=22.23.3
ARG NODE_SHA256_AMD64=df450af89261115ef9f9e3830c3eeb2cc9213b63c720b1af623cb5dcbe2e02de
ARG NODE_SHA256_ARM64=a44aeb94849a299b22df10b9e622ec2f605c2183501bc40590705131de7c740f

ENV CARGO_HOME=/opt/cargo \
    RUSTUP_HOME=/opt/rustup \
    PATH=/opt/cargo/bin:/opt/node/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin

# gcc + libc6-dev: the native linker. wasm32 links with the bundled rust-lld.
# With the extra CA present, apt goes over https for this one step (such proxies
# refuse plain http), then the sources are put back exactly as the base shipped them.
RUN --mount=type=secret,id=extra_ca,required=false,mode=0444 \
    set -eu; \
    src=/etc/apt/sources.list.d/debian.sources; \
    if [ -s /run/secrets/extra_ca ]; then \
      sed -i 's#http://#https://#' "$src"; \
      echo 'Acquire::https::CAInfo "/run/secrets/extra_ca";' > /etc/apt/apt.conf.d/99extra-ca; \
    fi; \
    apt-get update; \
    apt-get install -y --no-install-recommends ca-certificates curl xz-utils gcc libc6-dev; \
    sed -i 's#https://#http://#' "$src"; \
    rm -rf /var/lib/apt/lists/* /etc/apt/apt.conf.d/99extra-ca

# Checksums are verified before anything downloaded is executed or unpacked.
RUN --mount=type=secret,id=extra_ca,required=false,mode=0444 \
    set -eu; \
    if [ -s /run/secrets/extra_ca ]; then \
      export SSL_CERT_FILE=/run/secrets/extra_ca CURL_CA_BUNDLE=/run/secrets/extra_ca; \
    fi; \
    case "$(dpkg --print-architecture)" in \
      amd64) triple=x86_64-unknown-linux-gnu; rsum=$RUSTUP_SHA256_AMD64; narch=x64; nsum=$NODE_SHA256_AMD64 ;; \
      arm64) triple=aarch64-unknown-linux-gnu; rsum=$RUSTUP_SHA256_ARM64; narch=arm64; nsum=$NODE_SHA256_ARM64 ;; \
      *) echo "unsupported architecture" >&2; exit 1 ;; \
    esac; \
    curl -fsSLo /tmp/rustup-init "https://static.rust-lang.org/rustup/archive/${RUSTUP_VERSION}/${triple}/rustup-init"; \
    echo "${rsum}  /tmp/rustup-init" | sha256sum -c -; \
    chmod +x /tmp/rustup-init; \
    /tmp/rustup-init -y --no-modify-path --profile minimal \
      --default-toolchain "${RUST_VERSION}" --component rustfmt,clippy \
      --target wasm32-unknown-unknown; \
    rm /tmp/rustup-init; \
    curl -fsSLo /tmp/node.tar.xz "https://nodejs.org/dist/v${NODE_VERSION}/node-v${NODE_VERSION}-linux-${narch}.tar.xz"; \
    echo "${nsum}  /tmp/node.tar.xz" | sha256sum -c -; \
    mkdir -p /opt/node; \
    tar -xJf /tmp/node.tar.xz -C /opt/node --strip-components=1; \
    rm -rf /tmp/node.tar.xz /opt/node/include /opt/node/share "${CARGO_HOME}/registry"

WORKDIR /w

CMD ["cargo", "--version"]
