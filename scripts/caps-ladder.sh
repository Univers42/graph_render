#!/usr/bin/env bash
# The service-caps ladder (docs/measurements/service-caps.md). For each layout and post
# id, `graph-cli cap-probe` runs at n = 256, 512, ... up to the id's scale_ceiling (or
# 2^20), one process per rung, each under `timeout 40`. Run it inside the rust image,
# one measurement process at a time, after a release build:
#
#   scripts/orch/gr cargo build --release -p graph-cli
#   scripts/orch/gr sh -c 'target/release/graph-cli capabilities --json > caps.json'
#   scripts/orch/gr bash scripts/caps-ladder.sh caps.json ladder.log [id ...]
#
# - Without ids, every `layout.*` and `post.*` row of caps.json is measured, in its order.
# - A rung over 15 s, a rung killed or failed, or 180 s spent on the id ends its ladder.
# - Posts count their ceiling in edges, so a post's top rung is the n whose seeded model has
#   about that many edges (m ≈ 1.55 n). A post runs over each layout `inputs` names.
# - After the sparse ladder, one dense rung (`--edges-per-node 4`) runs at the largest
#   sparse rung that finished within 15 s.
#
# Caveat: one run per rung, on whatever load the host carries, so a rung near 15 s can land
# on either side of it from one run to the next. The ladder assumes cost grows with n: a
# size past the first rung over 15 s is never tried, and a rung between two sizes is never
# tried either, so the largest size under 15 s can be up to twice the one reported.
set -euo pipefail

caps=$1 log=$2
shift 2
probe=target/release/graph-cli
limit_ms=15000 budget_s=180 top=$((1 << 20))
last_ms=''

ceilings() {
	awk -F'"' '/^    "id": /{id=$4} /^    "scale_ceiling": /{gsub(/[^0-9]/,"",$3); print id, $3}' "$caps"
}

min() { echo $(($1 < $2 ? $1 : $2)); }

# The layouts whose geometry a post is timed over. The node mover moves only circles and
# boxes, so it is timed over the two layouts that make them.
inputs() {
	case $1 in
	post.separate.grid) echo layout.packing.circle layout.treemap.squarified ;;
	*) echo layout.circular.radial layout.grid ;;
	esac
}

# One rung, logged; `last_ms` is its timed window, or empty when there is no number.
rung() {
	local id=$1 n=$2 code=0 line
	shift 2
	line=$(timeout 40 "$probe" cap-probe --id "$id" --n "$n" "$@" 2>&1 </dev/null) || code=$?
	last_ms=''
	if [ "$code" = 0 ]; then
		last_ms=$(sed -n 's/.* ms=\([0-9.]*\) .*/\1/p' <<<"$line")
	elif [ "$code" = 124 ]; then
		line="cap-probe id=$id n=$n $* status=killed reason=over-40s"
	else
		line="cap-probe id=$id n=$n $* status=failed exit=$code ${line//$'\n'/ }"
	fi
	printf '%s\n' "$line" | tee -a "$log"
}

ladder() {
	local id=$1 high=$2 n=256 start=$SECONDS best=''
	shift 2
	while :; do
		n=$(min "$n" "$high")
		rung "$id" "$n" "$@"
		[ -n "$last_ms" ] || break
		awk -v ms="$last_ms" -v lim="$limit_ms" 'BEGIN { exit !(ms > lim) }' && break
		best=$n
		[ "$n" -lt "$high" ] || break
		[ $((SECONDS - start)) -lt "$budget_s" ] || break
		n=$((n * 2))
	done
	[ -z "$best" ] || rung "$id" "$best" --edges-per-node 4 "$@"
}

wanted() { [ $# -eq 1 ] || printf '%s\n' "${@:2}" | grep -qxF "$1"; }

printf '# host: nproc %s, load %s, %s\n' "$(nproc)" "$(cut -d' ' -f1-3 /proc/loadavg)" \
	"$(sed -n 's/^model name[[:space:]]*: //p' /proc/cpuinfo | head -1)" | tee -a "$log"
ceilings | while read -r id ceiling; do
	wanted "$id" "$@" || continue
	case $id in
	layout.*) ladder "$id" "$(min "$ceiling" "$top")" ;;
	post.*)
		for input in $(inputs "$id"); do
			ladder "$id" "$(min $((ceiling * 100 / 155)) "$top")" --input "$input"
		done
		;;
	esac
done
