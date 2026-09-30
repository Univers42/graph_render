# syntax=docker/dockerfile:1
#
# The browser MCP server the OpenCode `ux` agent drives (opencode.json `mcp.pw`). A test
# instrument for the studio, never shipped and never a dependency of any package. Debian
# chromium and Debian node, no vendor browser image (docs/decisions/opencode-browser-mcp.md);
# @playwright/mcp is pinned by deploy/mcp-browser/package-lock.json and installed with `npm ci`.
#
#   docker build -f deploy/mcp-browser.Dockerfile -t gm-mcp-browser deploy
#   docker run -i --rm --network host --user 0:0 -v "$GM_SCRATCH/mcp-out:/out" gm-mcp-browser   # scripts/orch/pw-mcp.sh
#
# The image user is `mcp`. This host runs rootless docker, where container root is the invoking
# host user and uid 1000 is an unowned subuid that cannot write the /out bind mount, so
# opencode.json passes `--user 0:0`. On a rootful daemon, drop that flag.
#
# Behind a TLS-intercepting proxy, hand its CA bundle over as an optional secret:
#   docker build --secret id=extra_ca,src=/path/ca.crt -f deploy/mcp-browser.Dockerfile -t gm-mcp-browser deploy
#
# Ponytail: playwright-core is built against its own Chromium revision and drives Debian's
# instead. A protocol skew shows up as a failed browser_navigate in the OpenCode smoke run,
# not as a build error; bump the lockfile or the base image together when it does.

FROM debian:trixie-slim

RUN --mount=type=secret,id=extra_ca,required=false,mode=0444 \
    set -eu; \
    src=/etc/apt/sources.list.d/debian.sources; \
    if [ -s /run/secrets/extra_ca ]; then \
      sed -i 's#http://#https://#' "$src"; \
      echo 'Acquire::https::CAInfo "/run/secrets/extra_ca";' > /etc/apt/apt.conf.d/99extra-ca; \
    fi; \
    apt-get update; \
    apt-get install -y --no-install-recommends chromium nodejs npm ca-certificates \
      fonts-dejavu-core tini; \
    sed -i 's#https://#http://#' "$src"; \
    rm -rf /var/lib/apt/lists/* /etc/apt/apt.conf.d/99extra-ca

WORKDIR /opt/mcp
COPY mcp-browser/package.json mcp-browser/package-lock.json ./
RUN --mount=type=secret,id=extra_ca,required=false,mode=0444 \
    set -eu; \
    if [ -s /run/secrets/extra_ca ]; then export NODE_EXTRA_CA_CERTS=/run/secrets/extra_ca; fi; \
    PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD=1 npm ci --omit=dev --ignore-scripts --no-audit --no-fund

RUN chromium --version && node --version \
 && node node_modules/@playwright/mcp/cli.js --help > /dev/null

# The server talks MCP over stdio; the browser and its profile die with the container.
RUN useradd -m mcp && mkdir -p /out && chown mcp /out
USER mcp
# Relative screenshot names resolve against the working directory, not --output-dir.
WORKDIR /out
ENTRYPOINT ["tini", "--", "node", "/opt/mcp/node_modules/@playwright/mcp/cli.js", \
  "--headless", "--isolated", "--no-sandbox", "--browser", "chromium", \
  "--executable-path", "/usr/bin/chromium", "--output-dir", "/out"]
