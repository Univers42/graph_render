#!/usr/bin/env bash
# pw-mcp.sh — OpenCode's browser MCP server `pw` (opencode.json): Playwright MCP in the gm-mcp-browser
# image (deploy/mcp-browser.Dockerfile, docs/decisions/opencode-browser-mcp.md). A script, not an
# inline command, because the screenshot mount needs GM_SCRATCH and a usable DOCKER_HOST, which
# OpenCode's config cannot compute. Screenshots land in $GM_SCRATCH/mcp-out (/out in the container).
# --pull never: a missing image fails the server instead of pulling a stranger's image of that name.
# Enabled for every agent since 2026-09-30. An `Unknown tool 'pw.…'` on the first call is the
# catalog being built before the container connected (about 4 s): search again, then call.
set -euo pipefail
here=$(dirname "$(readlink -f "$0")")
source "$here/scratch.sh"
source "$here/docker-env.sh"
mkdir -p "$GM_SCRATCH/mcp-out"
origins="http://127.0.0.1:5173;http://127.0.0.1:5174;http://127.0.0.1:5175"
origins+=";http://localhost:5173;http://localhost:5174;http://localhost:5175"
exec docker run --pull never -i --rm --network host --user 0:0 -v "$GM_SCRATCH/mcp-out:/out" \
  gm-mcp-browser --allowed-origins "$origins"
