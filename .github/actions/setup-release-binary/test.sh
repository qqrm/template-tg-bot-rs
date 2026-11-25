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
asset="${3##*/}"
printf '%s\n' "$asset" >> "$FIXTURES/downloads"
cp "$FIXTURES/$asset" "$out"
MOCK
chmod +x bin/curl
sha=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
other=bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
printf 'test binary\n' > fixtures/bot
hash="$(sha256sum fixtures/bot | cut -d' ' -f1)"
manifest() {
  jq -n --arg sha "$1" --arg hash "$2" --argjson protocol "$3" \
    '{source_sha:$sha,sha256:$hash,protocol:$protocol}' > fixtures/delivery-manifest.json
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
jq -n --arg hash "$hash" '{sha256:$hash,protocol:2}' > fixtures/delivery-manifest.json
reject
printf 'not json' > fixtures/delivery-manifest.json
reject
manifest "$sha" "${hash:0:63}" 2
reject
manifest "$sha" "$(printf '%064d' 0)" 2
reject
manifest "$sha" "$hash" 2
rm -f fixtures/bot
reject
printf 'test binary\n' > fixtures/bot
bash "$setup" example/repo bot "$sha"
test -x bot
cmp bot fixtures/bot
cmp delivery-manifest.json fixtures/delivery-manifest.json
printf 'Release selection regression tests passed.\n'
