#!/usr/bin/env sh
# git-log.sh <repo>: the plugin's one git call, on the host's git. Children first, every ref.
# One commit per line: %s is the subject's first line, so a record never spans two lines.
set -eu
exec git -C "$1" log --all --topo-order --format='%H%x1f%P%x1f%an%x1f%at%x1f%D%x1f%s'