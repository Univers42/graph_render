#!/usr/bin/env bash
# fetch-refs.sh [dest] — download the pinned read-only references (prompts/REFERENCES.md) into
# <dest> (default $GM_SCRATCH/refs, scripts/orch/scratch.sh), verify each against its recorded digest, chmod a-w.
# Digests: npm sha512 = the `integrity` in the branch lockfiles; networkx, igraph and matplotlib's
# _cm_listed.py = PyPI's published sha256 (igraph is the PyPI name of python-igraph since 0.10, so
# 0.11.9 resolves to the sdist whose own sha256 is recorded below; matplotlib's file comes from
# inside the 3.10.0 sdist, sha256 b886d02a…511278 checked 2026-09-29, fetched alone because the
# sdist is 36 MB); JAMA and lobpcg.py = the sha256 recorded on p6e (dense_sym.rs:5-6,
# eigensolver.md). A mismatch is a stop (rule 0.6), never a retry against another mirror.
set -euo pipefail
source "$(dirname "$(readlink -f "$0")")/scratch.sh"
R=${1:-$GM_SCRATCH/refs}
die() { echo "fetch-refs: $*" >&2; exit 1; }
sha256_is() { [[ $(sha256sum "$1" | cut -d' ' -f1) == "$2" ]] || die "sha256 mismatch: $1"; }
get() { curl -fsSL --retry 3 -o "$2" "$1"; }
npm_ref() { # pkg ver integrity
  local d=$R/npm/$1-$2 t
  [[ -d $d/package ]] && return 0
  mkdir -p "$d"; t=$d/$1-$2.tgz; get "https://registry.npmjs.org/$1/-/$1-$2.tgz" "$t"
  [[ "sha512-$(openssl dgst -sha512 -binary "$t" | base64 -w0)" == "$3" ]] || die "sha512 mismatch: $t"
  tar -xzf "$t" -C "$d"
}
[[ -w ${R%/*} || -w $R ]] || die "cannot write $R"
mkdir -p "$R/npm"; chmod -R u+w "$R"

npm_ref d3-hierarchy 3.1.2 'sha512-FX/9frcub54beBdugHjDCdikxThEqjnR93Qt7PvQTOHxyiNCAlvMrHhclk3cD5VeAaq9fxmfRp+CnWw9rEMBuA=='
npm_ref dagre-d3-es 7.0.14 'sha512-P4rFMVq9ESWqmOgK+dlXvOtLwYg0i7u0HBGJER0LZDJT2VHIPAMZ/riPxqJceWMStH5+E61QxFra9kIS3AqdMg=='
npm_ref d3-force 3.0.0 'sha512-zxV/SsA+U4yte8051P4ECydjD/S+qeYtnaIyAs9tgHCqfguma/aAQDjo85A9Z6EKhBirHRJHXIgJUlffT4wdLg=='
npm_ref d3-quadtree 3.0.1 'sha512-04xDrxQTDTCFwP5H6hRhsRcb9xxv2RzkcsygFzmkSIOJy3PeRJP7sNk3VRIbKXcog561P9oU0/rVH6vDROAgUw=='

pypi_ref() { # pkg ver sha256: the sdist into $R/<pkg>-<ver>/, extracted beside its tarball
  local d=$R/$1-$2 t=$R/$1-$2/$1-$2.tar.gz meta url
  if [[ ! -d $d/$1-$2 ]]; then
    mkdir -p "$d"; meta=$(curl -fsSL "https://pypi.org/pypi/$1/$2/json")
    url=$(jq -r '.urls[] | select(.packagetype=="sdist") | .url' <<<"$meta")
    [[ $(jq -r '.urls[] | select(.packagetype=="sdist") | .digests.sha256' <<<"$meta") == "$3" ]] \
      || die "PyPI digest changed for $1 $2"
    get "$url" "$t"; sha256_is "$t" "$3"
    tar -xzf "$t" -C "$d"
  fi
  sha256_is "$t" "$3"
}

pypi_ref networkx 3.6 285276002ad1f7f7da0f7b42f004bcba70d381e936559166363707fdad3d72ad
# `igraph`, not `python-igraph`: the latter's 0.11.9 sdist is a 9.7 kB shim depending on this one.
pypi_ref igraph 0.11.9 c57ce44873abcfcfd1d61d7d261e416d352186958e7b5d299cf244efa6757816

gv=$R/graphviz-16.1.0
if [[ ! -f $gv/graphviz-16.1.0.tar.gz ]]; then
  mkdir -p "$gv"
  get https://gitlab.com/api/v4/projects/4207231/packages/generic/graphviz-releases/16.1.0/graphviz-16.1.0.tar.gz "$gv/graphviz-16.1.0.tar.gz"
  sha256_is "$gv/graphviz-16.1.0.tar.gz" beea483ab130f456c1c3905f4f2e40778a9c493c3d73ae8012367d552d71ca84
  tar -xzf "$gv/graphviz-16.1.0.tar.gz" -C "$gv"
fi

jama=$R/jama-1.0.3
if [[ ! -f $jama/Jama/EigenvalueDecomposition.java ]]; then
  mkdir -p "$jama"; get https://repo1.maven.org/maven2/gov/nist/math/jama/1.0.3/jama-1.0.3-sources.jar "$jama/jama-1.0.3-sources.jar"
  sha256_is "$jama/jama-1.0.3-sources.jar" 5691d17d91efb78697cf7470f10b319cafa6be2fbbc1fa98b978e11ce9fd9356
  (cd "$jama" && unzip -q -o jama-1.0.3-sources.jar)
fi
sha256_is "$jama/Jama/EigenvalueDecomposition.java" c9d8efe5cfd22b7dddfc3d7fb185bb01d1a6ed2130610c0fcfe7195ccee36a81

sp=$R/scipy-1.16.2
if [[ ! -f $sp/lobpcg.py ]]; then
  mkdir -p "$sp"; get https://raw.githubusercontent.com/scipy/scipy/v1.16.2/scipy/sparse/linalg/_eigen/lobpcg/lobpcg.py "$sp/lobpcg.py"
fi
sha256_is "$sp/lobpcg.py" 09d3378487b4466ee97dd5c0225b7e75d0268498d32c91b1e054f69a0253b88b

mpl=$R/matplotlib-3.10.0
if [[ ! -f $mpl/_cm_listed.py ]]; then
  mkdir -p "$mpl"; get https://raw.githubusercontent.com/matplotlib/matplotlib/v3.10.0/lib/matplotlib/_cm_listed.py "$mpl/_cm_listed.py"
fi
sha256_is "$mpl/_cm_listed.py" ddad3698f5129ceb1792a445371286c08bc9080298e657b3054aea19c9659ef9

chmod -R a-w "$R"
echo "fetch-refs: 10 references verified under $R"
