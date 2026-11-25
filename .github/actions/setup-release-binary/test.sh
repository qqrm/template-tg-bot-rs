#!/usr/bin/env bash
set -euo pipefail
setup="$(cd "$(dirname "$0")" && pwd)/setup.sh"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
cd "$work"
mkdir fixtures bin
export FIXTURES="$work/fixtures"
export PATH="$work/bin:$PATH"
export RELEASE_WAIT_SECONDS=0
cat > bin/curl <<'MOCK'
#!/usr/bin/env bash
set -euo pipefail
while [[ "$1" != -o ]]; do shift; done
out="$2"
url="$3"
asset="${url##*/}"
tag="${url%/*}"
tag="${tag##*/}"
[[ "$url" == "https://github.com/example/repo/releases/download/$tag/$asset" ]]
[[ "$tag" =~ ^delivery-[0-9a-f]{40}$ ]]
printf '%s\n' "$asset" >> "$FIXTURES/downloads"
cp "$FIXTURES/$tag/$asset" "$out"
MOCK
chmod +x bin/curl
sha=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
other=bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
mkdir -p "fixtures/delivery-$sha" "fixtures/delivery-$other"
fixture="fixtures/delivery-$sha"
printf 'test binary\n' > "$fixture/bot"
hash="$(sha256sum "$fixture/bot" | cut -d' ' -f1)"
manifest() {
  jq -n --arg sha "$1" --arg hash "$2" --argjson protocol "$3" \
    '{source_sha:$sha,sha256:$hash,protocol:$protocol}' > "$fixture/delivery-manifest.json"
}
reject() {
  rm -f bot delivery-manifest.json fixtures/downloads
  if bash "$setup" example/repo bot "$sha"; then
    echo 'Unexpected acceptance of invalid release' >&2
    exit 1
  fi
  test ! -e bot
  test ! -e delivery-manifest.json
}
manifest "$other" "$hash" 2
reject
# A valid hash from a stale commit must not even trigger a binary download.
[[ "$(cat fixtures/downloads)" == delivery-manifest.json ]]
manifest "$sha" "$hash" 1
reject
jq -n --arg hash "$hash" '{sha256:$hash,protocol:2}' > "$fixture/delivery-manifest.json"
reject
printf 'not json' > "$fixture/delivery-manifest.json"
reject
manifest "$sha" "${hash:0:63}" 2
reject
manifest "$sha" "$(printf '%064d' 0)" 2
reject
manifest "$sha" "$hash" 2
rm -f "$fixture/bot"
reject
printf 'test binary\n' > "$fixture/bot"
bash "$setup" example/repo bot "$sha"
test -x bot
cmp bot "$fixture/bot"
cmp delivery-manifest.json "$fixture/delivery-manifest.json"
# A newer release must not displace an older in-flight checkout.
printf 'newer binary\n' > "fixtures/delivery-$other/bot"
other_hash="$(sha256sum "fixtures/delivery-$other/bot" | cut -d' ' -f1)"
jq -n --arg sha "$other" --arg hash "$other_hash" \
  '{source_sha:$sha,sha256:$hash,protocol:2}' > "fixtures/delivery-$other/delivery-manifest.json"
bash "$setup" example/repo bot "$other"
cmp bot "fixtures/delivery-$other/bot"
bash "$setup" example/repo bot "$sha"
cmp bot "$fixture/bot"

# A missing commit release must not fall back to any other release.
missing=cccccccccccccccccccccccccccccccccccccccc
rm -f bot delivery-manifest.json
if bash "$setup" example/repo bot "$missing"; then
  echo 'Unexpected fallback to another commit release' >&2
  exit 1
fi
test ! -e bot
test ! -e delivery-manifest.json
printf 'Release selection regression tests passed.\n'
