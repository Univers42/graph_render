# shellcheck shell=bash
# Sourced by the wrappers that run a house image. On a fresh host (goinfre wiped, 2026-10-02)
# `scripts/studio.sh` died on `pull access denied for ge-rust`: nothing built the image, and
# docker went looking for it on Docker Hub. Every image here is built from its committed recipe
# instead, on first use, and rebuilt when the recipe changes.
#
# GM_NODE_IMAGE: the node image every node container runs, pinned by digest. node:22-slim is a
# moving tag; 22.23.3 is the release docker/rust.Dockerfile installs for the hashgate's wasm arm.
export GM_NODE_IMAGE=${NODE_IMAGE:-node:22.23.3-slim@sha256:43ac6c60b8f89723f746e8a92ce91abd5017e627ce1ddfe4238355d3a30b772c}
gm_image_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
source "$gm_image_root/scripts/orch/scratch.sh"
source "$gm_image_root/scripts/orch/docker-env.sh"

# gm_image_recipe <tag> — "<dockerfile> <base>" for an image built from the repo, else nothing.
# None of these recipes COPY from a build context, so they build from an empty one: the repo
# context would send target/ (gigabytes) to the daemon for nothing.
gm_image_recipe() {
  case $1 in
    ge-rust) echo "docker/rust.Dockerfile -" ;;
    ge-mutants) echo "docker/mutants.Dockerfile ge-rust" ;;
    ge-profile) echo "docker/profile.Dockerfile ge-rust" ;;
    ge-audit) echo "docker/audit.Dockerfile ge-rust" ;;
    ge-wasm-threads) echo "docker/wasm-threads.Dockerfile ge-rust" ;;
    gm-chromium) echo "deploy/chromium.Dockerfile -" ;;
    gm-media) echo "deploy/media.Dockerfile -" ;;
  esac
}

# ensure_image <tag> — build <tag> when the host lacks it or holds one built from another recipe.
# An unknown tag is left to `docker run --pull never`, which names it when it is missing.
#
# Caveat: the label proves the recipe, not the bytes. The recipes start from debian:trixie-slim
# and apt, both moving, so two hosts with equal labels can still hold different packages; an
# image built by hand from its header line has no label and is rebuilt once (from the layer
# cache when the host has one). GM_EXTRA_CA=<ca.crt> passes a TLS-intercepting proxy's CA.
ensure_image() {
  local tag=$1 file base recipe
  read -r file base <<<"$(gm_image_recipe "$tag")"
  [[ -n $file ]] || return 0
  if [[ $base != - ]]; then ensure_image "$base" || return; fi
  recipe=$({ cat "$gm_image_root/$file"; [[ $base == - ]] || docker image inspect -f '{{.Id}}' "$base"; } | sha256sum)
  recipe=${recipe%% *}
  [[ $(gm_image_label "$tag") == "$recipe" ]] && return 0
  gm_image_build "$tag" "$file" "$recipe"
}

gm_image_label() {
  docker image inspect -f '{{index .Config.Labels "gm.recipe"}}' "$1" 2>/dev/null
}

# One build per tag at a time: two wrappers started together on a fresh host would both build.
gm_image_build() {
  local tag=$1 file=$2 recipe=$3 secret=() context
  [[ -n ${GM_EXTRA_CA-} ]] && secret=(--secret "id=extra_ca,src=$GM_EXTRA_CA")
  mkdir -p "$GM_SCRATCH/locks"
  (
    flock 9
    [[ $(gm_image_label "$tag") == "$recipe" ]] && exit 0
    printf '[image] building %s from %s (once per recipe)\n' "$tag" "$file" >&2
    context=$(mktemp -d)
    trap 'rmdir "$context"' EXIT
    docker build --label "gm.recipe=$recipe" ${secret[@]+"${secret[@]}"} \
      -f "$gm_image_root/$file" -t "$tag" "$context" >&2
  ) 9>"$GM_SCRATCH/locks/image-$tag.lock"
}
