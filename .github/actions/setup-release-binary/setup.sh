#!/usr/bin/env bash
set -euo pipefail

repository="$1"
binary_name="$2"
source_sha="$3"
[[ "$source_sha" =~ ^[0-9a-f]{40}$ ]]
wait_seconds="${RELEASE_WAIT_SECONDS:-900}"
[[ "$wait_seconds" =~ ^[0-9]+$ ]]

tmp_dir="$(mktemp -d)"
trap 'rm -rf "$tmp_dir"' EXIT
manifest="$tmp_dir/delivery-manifest.json"
binary="$tmp_dir/$binary_name"
base_url="https://github.com/$repository/releases/download/delivery-$source_sha"
download() {
  curl --fail --silent --show-error --location --max-time 20 -o "$1" "$2"
}

deadline=$((SECONDS + wait_seconds))
while :; do
  # Do not download or execute a binary for a different source commit.
  if download "$manifest" "$base_url/delivery-manifest.json" &&
     jq -e --arg source_sha "$source_sha" '
       .protocol == 2 and .source_sha == $source_sha and
       (.sha256 | type == "string" and test("^[0-9a-f]{64}$"))
     ' "$manifest" >/dev/null 2>&1 &&
     download "$binary" "$base_url/$binary_name"; then
    expected="$(jq -r '.sha256' "$manifest")"
    actual="$(sha256sum "$binary" | cut -d' ' -f1)"
    if [[ "$actual" == "$expected" ]]; then
      chmod +x "$binary"
      mv -f -- "$binary" "$binary_name"
      mv -f -- "$manifest" delivery-manifest.json
      exit 0
    fi
  fi
  # A commit-specific release may not yet be visible during publication.
  remaining=$((deadline - SECONDS))
  if (( remaining <= 0 )); then
    echo "::error::No verified release binary for source_sha=$source_sha; refusing to run." >&2
    exit 1
  fi
  sleep "$(( remaining < 30 ? remaining : 30 ))"
done
