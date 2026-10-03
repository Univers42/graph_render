#!/usr/bin/env bash
# drun-check.sh [dir] — exit 1 when a shell script or a rows file under <dir> (default: the git
# top-level) starts a container with a bare `docker run` instead of scripts/orch/drun, which is what
# puts it under the aggregate memory ceiling (gm-slice.sh). Exit 0 when there is none.
#
# Caveat: a line match. It misses a call spelled another way (`$docker run`, `docker container run`,
# a `docker \` continuation) and skips only lines that start with `#`, so a trailing comment
# that mentions the command is a false hit. Python and Dockerfiles are not scanned: the harness
# only names the command in docstrings.
set -uo pipefail
dir=${1:-$(git rev-parse --show-toplevel)}
hits=$(grep -rnE --include='*.sh' --include='*.rows' --include=gr \
  --exclude-dir=.git --exclude-dir=node_modules --exclude-dir=target --exclude-dir=SciGraphs \
  --exclude-dir=.claude '(^|[^-[:alnum:]_/])docker run( |$)' "$dir" | grep -vE '^[^:]+:[0-9]+:[[:space:]]*#')
if [[ -n $hits ]]; then
  printf '%s\n' "$hits"
  echo "drun-check: bare docker-run call above; use scripts/orch/drun" >&2
  exit 1
fi
echo "drun-check: every container goes through drun"
