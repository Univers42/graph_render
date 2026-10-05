#!/usr/bin/env bash
#
# forbidden-constructs.sh — the phase-11 "no forbidden constructs" gate row, scoped to
# PRODUCT code.
#
# The row's rule is a rule about what graph-core *computes*: no `mul_add` (D2, because
# FMA changes the rounding and breaks native/wasm32 bit-identity), no `relaxed`/FTZ/DAZ, no
# `rayon` (the crate may not grow a dependency that schedules), no `std::thread` (the
# executors live in their hosts, `compute-tiers.md` rule 2), no `Instant::now` (no
# wall-clock, D8), and no `libm::sqrt`: libm 0.2 runs a software square root on wasm32,
# and `f64::sqrt` is IEEE-754's correctly rounded one on both targets, so the same bytes
# (`docs/measurements/perf-p3-sqrt.md`).
#
# The unscoped form of that row — `grep -rnE "...pattern..." crates/graph-core/src` — cannot
# be used, because it matches three things that are not the rule:
#
#   1. **doc comments**, which name the forbidden constructs in order to forbid them.
#      `exec/partition.rs` says "the executors live in their hosts — `std::thread::scope`
#      in graph-cli", and `styles.rs` says "never `std`; no `mul_add`". Those lines are the
#      rule being *documented*, and a row that reads them as violations would force the
#      documentation to be deleted to get a green gate.
#   2. **`#[cfg(test)]` modules.** `post/routed/measure.rs` is an `#[ignore]`d benchmark
#      and `linalg/lobpcg/tests.rs` is test code. Neither is compiled into the product.
#   3. **`tests.rs` and `tests/` files**, which are the test module by convention.
#
# So this script scans only product code, and only its non-comment lines.
#
# Usage:  forbidden-constructs.sh [--self-test] [ROOT]
#         ROOT defaults to crates/graph-core/src.
#
# Exit codes: 0 no forbidden construct in product code · 1 one was found · 2 the check
#             could not run (a usage error, or a root that does not exist).
#
# --self-test runs the check against a throwaway tree holding one planted violation, which
# is the negative control for the row itself: a checker that cannot go red proves nothing,
# and the row is only worth its cost if it can.

set -uo pipefail

# `--self-test` is the only flag; anything else in `$1` is the root. A path that merely
# *starts* with a dash would need `--`, which no caller here uses.
self_test=0
if [[ ${1:-} == --self-test ]]; then
  self_test=1
  shift
fi
root=${1:-crates/graph-core/src}
case ${1:-} in
  "") ;;
  -h | --help) sed -n '3,28p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
  -*) echo "forbidden-constructs.sh: unknown option '$1'" >&2; exit 2 ;;
esac

# The forbidden constructs, as one extended regex. Each alternative is the *token* a
# violation would contain; the comment filter below is what keeps the prose that names them
# from matching. Relaxed SIMD is matched by its tokens, `relaxed-simd` and `f32x4_relaxed_*`:
# the bare word matched prose inside a string literal ("the picture is merely less relaxed",
# registry/three_d/graph.rs, gate-develop-178c, 2026-10-04).
pattern='mul_add|relaxed[-_]|rayon|std::thread|Instant::now|libm::sqrt'

# Every `mod NAME;` declared under `#[cfg(test)]`, across the whole tree: a `cfg(test)`
# module's *contents* are not product code even though the file's name looks ordinary
# (`post/routed/measure.rs` is the case that motivated this script). Read with a two-file
# window so the attribute and the declaration may sit on one line or on two.
cfg_test_modules() {
  find "$root" -name '*.rs' -print0 | xargs -0 awk '
    { line[NR] = $0; file[NR] = FILENAME }
    END {
      for (i = 1; i <= NR; i++) {
        if (line[i] !~ /#\[cfg\(test\)\]/) continue
        # The declaration is whatever follows the attribute: the rest of this line, or one
        # of the next two when the attribute sits alone above it. A line that is nothing
        # but another attribute is stepped over, never taken as the declaration.
        rest = line[i]
        sub(/.*#\[cfg\(test\)\]/, "", rest)
        for (j = i; j <= i + 3 && j <= NR; j++) {
          text = (j == i) ? rest : line[j]
          if (text ~ /^[[:space:]]*$/ || text ~ /^[[:space:]]*#\[/) continue
          if (match(text, /mod[[:space:]]+[A-Za-z_][A-Za-z0-9_]*/)) {
            name = substr(text, RSTART, RLENGTH)
            sub(/^mod[[:space:]]+/, "", name)
            # Rust resolves a module declaration against the directory named by the
            # declaring file: `post/routed.rs` declaring `mod measure;` means
            # `post/routed/measure.rs`, not `post/measure.rs`; and `sub/mod.rs` declaring
            # `mod kernels;` means `sub/kernels.rs`, not `sub/mod/kernels.rs`.
            base = file[i]
            sub(/\.rs$/, "", base)
            sub(/\/mod$/, "", base)
            print base "/" name ".rs"
            print base "/" name "/"
          }
          break
        }
      }
    }' 2>/dev/null
}

# The files to scan: every `.rs` under `root`, minus the test modules, minus the `cfg(test)`
# ones the two-file window found. Sorted, so a violation is reported in a stable order.
product_files() {
  local skip
  skip=$(cfg_test_modules)
  find "$root" -name '*.rs' | sort | while read -r file; do
    # A `cfg(test)` module's own file or directory.
    if printf '%s\n' "$skip" | grep -qxF "$file"; then continue; fi
    # The test module by convention: `tests.rs`, and anything under a `tests/` directory.
    case $file in
      */tests.rs | */tests/*) continue ;;
    esac
    printf '%s\n' "$file"
  done
}

# The scan itself. Comment lines are dropped rather than matched: a Rust file's own
# documentation is the only place these names legitimately appear, and no forbidden
# construct can hide inside a comment (comments do not compile).
scan() {
  product_files | xargs grep -nE "$pattern" 2>/dev/null \
    | grep -vE ':[[:space:]]*(//|/\*|\*)' \
    | grep -vE ':[[:space:]]*//!' \
    || true
}

# The planted tree for --self-test: the product violations (`Instant::now`, `libm::sqrt`, two
# relaxed-SIMD tokens), the word `relaxed` in a string literal, and the three shapes the unscoped
# row used to match, so the test also shows the filter is what silences them.
self_test_tree() {
  local dir
  dir=$(mktemp -d)
  mkdir -p "$dir/src/sub" "$dir/src/sub/tests"
  cat >"$dir/src/lib.rs" <<'RS'
//! A doc comment naming Instant::now, mul_add, rayon and std::thread, as the prose does.
pub mod sub;
RS
  cat >"$dir/src/ok.rs" <<'RS'
//! no mul_add here either
pub const NOTE: &str = "the picture is merely less relaxed";
pub fn add(a: f64, b: f64) -> f64 {
    a + b
}
RS
  cat >"$dir/src/simd.rs" <<'RS'
#[target_feature(enable = "relaxed-simd")]
pub fn fused(a: v128, b: v128, c: v128) -> v128 {
    f32x4_relaxed_madd(a, b, c)
}
RS
  cat >"$dir/src/bad.rs" <<'RS'
pub fn tick() -> std::time::Duration {
    let start = Instant::now();
    start.elapsed()
}
RS
  cat >"$dir/src/slow.rs" <<'RS'
pub fn norm(a: f64, b: f64) -> f64 {
    libm::sqrt(a * a + b * b)
}
RS
  # Product-visible violation behind a test-only module, which must be ignored.
  cat >"$dir/src/sub/mod.rs" <<'RS'
#[cfg(test)]
mod measure;
#[cfg(test)]
mod kernels;
pub mod fine;
RS
  cat >"$dir/src/sub/measure.rs" <<'RS'
pub fn timed() -> f64 {
    let t = Instant::now();
    t as f64
}
RS
  cat >"$dir/src/sub/kernels.rs" <<'RS'
pub fn mul_add(a: f64, b: f64, c: f64) -> f64 {
    a.mul_add(b, c)
}
RS
  cat >"$dir/src/sub/fine.rs" <<'RS'
pub fn scale(a: f64, b: f64) -> f64 {
    a * b
}
RS
  # Test files by convention, which must be ignored.
  cat >"$dir/src/sub/tests.rs" <<'RS'
pub fn relaxed() -> f64 {
    let t = Instant::now();
    t as f64
}
RS
  cat >"$dir/src/sub/tests/deep.rs" <<'RS'
pub fn rayon() -> f64 {
    1.0
}
RS
  printf '%s\n' "$dir"
}

if [[ $self_test == 1 ]]; then
  dir=$(self_test_tree)
  trap 'rm -rf "$dir"' EXIT
  # The throwaway tree, not the repository: a self-test that scanned the real crate would
  # pass for the wrong reason whenever the crate is clean.
  root=$dir/src
  hits=$(scan)
  status=0
  # The planted product violations must be found...
  if ! grep -q 'bad.rs' <<<"$hits"; then
    echo "forbidden-constructs.sh: self-test FAILED: the planted Instant::now was not found" >&2
    status=1
  fi
  if ! grep -q 'slow.rs' <<<"$hits"; then
    echo "forbidden-constructs.sh: self-test FAILED: the planted libm::sqrt was not found" >&2
    status=1
  fi
  if [[ $(grep -c 'simd.rs' <<<"$hits") != 2 ]]; then
    echo "forbidden-constructs.sh: self-test FAILED: the two planted relaxed-SIMD tokens were not both found" >&2
    status=1
  fi
  # ...and nothing else may be. Each of these is a shape the unscoped row used to match.
  for quiet in 'lib.rs' 'ok.rs' 'sub/measure.rs' 'sub/kernels.rs' 'sub/tests.rs' 'sub/tests/deep.rs'; do
    if grep -q "$quiet" <<<"$hits"; then
      echo "forbidden-constructs.sh: self-test FAILED: $quiet was scanned but must not be" >&2
      status=1
    fi
  done
  if [[ $status == 0 ]]; then
    echo "forbidden-constructs.sh: self-test ok (found bad.rs, slow.rs and simd.rs only)"
  fi
  exit $status
fi

if [[ ! -d $root ]]; then
  echo "forbidden-constructs.sh: no such directory: $root" >&2
  exit 2
fi

hits=$(scan)
if [[ -n $hits ]]; then
  echo "forbidden-constructs.sh: forbidden construct in product code under $root:" >&2
  echo "$hits" >&2
  exit 1
fi
exit 0
