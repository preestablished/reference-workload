#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/.." && pwd)
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT
mkdir -p "$TMP/bin" "$TMP/corpus/captures"
printf '{"ok":true}\n' > "$TMP/corpus/manifest.json"
printf '{"ok":true}\n' > "$TMP/corpus/captures/index.jsonl"
printf 'state-scorer-0gy closed\n' > "$TMP/resolution.md"

cat > "$TMP/bin/bn" <<'EOF'
#!/usr/bin/env bash
set -eu
if [ "$1" = project ]; then printf '{"name":"state-scorer"}\n'; exit 0; fi
if [ "$1" = list ]; then printf '%s\n' "${FAKE_SCORER_LIST:-[]}"; exit 0; fi
if [ "$1" = show ]; then
  id=$2
  status=${FAKE_STATUS:-closed}
  [ "$id" = refwork-czi ] && status=${FAKE_CZI_STATUS:-$status}
  [ "$id" = refwork-20v ] && status=${FAKE_20V_STATUS:-$status}
  [ "$id" = rom-operator-bridge-l1w ] && { printf '{"id":"%s","status":"closed"}\n' "$id"; exit 0; }
  case "${FAKE_MODE:-ok}" in wrong-id) printf '{"id":"wrong","status":"%s"}\n' "$status";; malformed) printf 'not-json\n';; *) printf '{"id":"%s","status":"%s"}\n' "$id" "$status";; esac
  exit 0
fi
exit 1
EOF
chmod +x "$TMP/bin/bn"

run_gate() {
  PATH="$TMP/bin:$PATH" M6_CORPUS_ROOT="$TMP/corpus" M6_SCORER_RESOLUTION="$TMP/resolution.md" "$ROOT/tools/m6-gate-check.sh" 2>&1
}
must_pass() { output=$(run_gate); printf '%s\n' "$output" | grep -q 'PASS     scorer-M3'; printf '%s\n' "$output" | grep -q 'PASS     refwork-czi'; }
must_fail() { if output=$(run_gate); then echo "expected failure" >&2; exit 1; fi; printf '%s\n' "$output" | grep -q "$1"; }

must_pass
FAKE_CZI_STATUS=open must_fail refwork-czi
FAKE_20V_STATUS=not-closed must_fail refwork-20v
FAKE_MODE=wrong-id must_fail refwork-czi
FAKE_MODE=malformed must_fail refwork-czi
FAKE_SCORER_LIST='[{"id":"state-scorer-0gy"},{"id":"state-scorer-0gy"}]' must_fail scorer-M3
echo 'm6 gate tests passed'
