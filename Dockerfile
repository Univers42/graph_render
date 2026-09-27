# syntax=docker/dockerfile:1
#
# The gate, containerised. osionos is Docker-first (no host node_modules, host Node
# is 20 while the repo needs 22) and this package inherits that: the reliable way
# to verify it is a pinned image, not whatever Node the host happens to carry.
#
#   docker build -t graph-engine-check .
#   docker run --rm graph-engine-check
#
# `npm ci`, not `npm install`: it installs exactly package-lock.yaml and fails loudly
# if the lock and package.json disagree, so a drifting transitive dep cannot quietly
# change what the gate checks. That is the whole reason the lockfile is committed.

FROM node:22-slim

WORKDIR /w

# Dependency layer first, so a source-only edit reuses the cached install.
COPY package.json package-lock.json ./
RUN npm ci

COPY tsconfig.json eslint.config.js ./
COPY src ./src
COPY tests ./tests

# Fail the build if the gate is red, instead of printing red and exiting 0.
CMD ["npm", "run", "check"]
