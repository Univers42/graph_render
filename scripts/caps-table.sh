#!/usr/bin/env bash
# Reduces caps-ladder.sh logs to the service caps (docs/measurements/service-caps.md):
#
#   scripts/caps-table.sh caps.json table ladder.log [rerun.log ...]   # the doc's rows
#   scripts/caps-table.sh caps.json tsv ladder.log [rerun.log ...]     # service-caps.tsv
#
# - Per id and input, cap n is the largest sparse rung within 15 s, below the first rung over
#   15 s, killed or failed.
# - Cap m is the dense rung's m when that rung, run at cap n, is within 15 s too; else it is
#   the sparse rung's m. A post's cap m is also at most its scale_ceiling, which counts edges.
# - A post's cap is the smaller over its inputs, and its peak the larger.
# - An id with no rung within 15 s gets cap 0 0, marked in the table.
# - An id found in a later log is taken from the last log that has it, whole: that is how a
#   re-run replaces a row. Rows come out in caps.json's order.
#
# Caveat: the log does not say which rung is the dense one, so a rung with m >= 3n is taken
# as dense. The seeded model's m/n is 1.52 to 1.6 at every size the ladder runs and the dense
# rung's is 4, so the split is exact on a ladder log, and wrong on a log of any other model.
set -euo pipefail

caps=$1 mode=$2
shift 2

awk -v mode="$mode" -v lim=15000 -v lim30=30000 '
function value(line, key,   at, rest) {
	at = index(line, " " key "=")
	if (!at) return ""
	rest = substr(line, at + length(key) + 2)
	sub(/ .*/, "", rest)
	return rest
}
function flag(line, name,   at, rest) {
	at = index(line, " " name " ")
	if (!at) return ""
	rest = substr(line, at + length(name) + 2)
	sub(/ .*/, "", rest)
	return rest
}
# Cut at a space, so a multibyte character is never split: mawk counts bytes.
function short(text,   cut) {
	sub(/[;:,] .*/, "", text)
	if (length(text) <= 48) return text
	cut = substr(text, 1, 46)
	sub(/ [^ ]*$/, "", cut)
	return cut "..."
}
FNR == NR {
	if ($0 ~ /^    "id": /) { split($0, a, "\""); id = a[4] }
	else if ($0 ~ /^    "scale_ceiling": /) { s = $0; gsub(/[^0-9]/, "", s); ceil[id] = s }
	else if ($0 ~ /^    "complexity": / && id ~ /^(layout|post)\./) {
		order[++ids] = id; split($0, a, "\""); cx[id] = short(a[4])
	}
	next
}
/^cap-probe / {
	id = value($0, "id")
	if (owner[id] != FILENAME) { owner[id] = FILENAME; count[id] = 0 }
	lines[id, ++count[id]] = $0
}
function rung(line,   id, input, n, m, key, dense, status) {
	id = value(line, "id"); n = value(line, "n") + 0; m = value(line, "m") + 0
	input = value(line, "input"); if (input == "") input = flag(line, "--input")
	if (input == "" || input == "-") input = "-"
	key = id SUBSEP input
	if (!(key in seen)) { seen[key] = 1; inputs[id] = inputs[id] " " input; over[key] = -1 }
	dense = flag(line, "--edges-per-node") != "" || (m && m >= 3 * n)
	status = value(line, "status")
	if (dense) { dstat[key] = status; dm[key] = m; dms[key] = value(line, "ms") + 0; dpk[key] = value(line, "peak_mib") + 0; return }
	ms[key, n] = value(line, "ms") + 0; mm[key, n] = m; pk[key, n] = value(line, "peak_mib") + 0
	ok[key, n] = status == "ok"; ns[key] = ns[key] " " n
	if ((status != "ok" || ms[key, n] > lim) && (over[key] < 0 || n < over[key])) over[key] = n
}
function settle(key,   list, i, n) {
	split(substr(ns[key], 2), list, " ")
	best[key] = ""; l30[key] = ""
	for (i in list) {
		n = list[i] + 0
		if (!ok[key, n]) continue
		if (ms[key, n] <= lim && (over[key] < 0 || n < over[key]) && n > best[key] + 0) best[key] = n
		if (ms[key, n] <= lim30 && n > l30[key] + 0) l30[key] = n
	}
}
function cap(id, key,   n) {
	settle(key); n = best[key] + 0
	cn = n; cm = mm[key, n] + 0; cpk = pk[key, n] + 0
	if (dstat[key] == "ok" && dms[key] <= lim) { cm = dm[key]; if (dpk[key] > cpk) cpk = dpk[key] }
	if (id ~ /^post\./ && cm > ceil[id] + 0) cm = ceil[id] + 0
}
function dense_text(key) {
	if (dstat[key] == "") return "not run"
	return dstat[key] == "ok" ? dm[key] " / " dms[key] : dstat[key]
}
function row(id,   list, i, key, n, m, peak, bind, via, kind, top) {
	for (i = 1; i <= count[id]; i++) rung(lines[id, i])
	n = -1; peak = 0; split(substr(inputs[id], 2), list, " ")
	for (i in list) {
		key = id SUBSEP list[i]; cap(id, key)
		if (cpk > peak) peak = cpk
		if (n < 0 || cn < n || (cn == n && cm < m)) { n = cn; m = cm; bind = key; via = list[i] }
	}
	if (n < 0) { n = 0; m = 0 }
	if (mode == "tsv") { print id "\t" n "\t" m; return }
	kind = id !~ /^post\./ ? "layout" : via == "" ? "post" : "post, over `" via "`"
	top = l30[bind]
	printf "| `%s` | %s | %s | %s | %s | %s | %s | %s | %s | %s |\n", id, kind, cx[id], ceil[id], \
		top == "" ? "none" : top " (" mm[bind, top] ")", ms[bind, top], pk[bind, top], \
		dense_text(bind), n == 0 ? "**0, unmeasured**" : n, m
	if (peak > worst) { worst = peak; worst_id = id }
}
END {
	if (mode == "tsv") print "id\tcap_n\tcap_m"
	for (i = 1; i <= ids; i++) row(order[i])
	if (mode != "tsv") printf "\nlargest peak at cap: %s MiB (`%s`)\n", worst, worst_id
}
' "$caps" "$@"
