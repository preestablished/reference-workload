#!/usr/bin/env bash
# M6 entry-gate checker. Read-only; malformed tracker data fails closed.
set -u
CORPUS_CANDIDATES=("${M6_CORPUS_ROOT:-$HOME/.agents/projects/reference-workload/corpus}" "$(dirname "$0")/../data/corpus")
RAW_SESSION_CANDIDATES=("${M6_RAW_SESSION_ROOT:-$HOME/.agents/projects/reference-workload/handplay-session}")
FALLBACK_CANDIDATES=("${M6_FALLBACK_ROOT:-$HOME/.agents/projects/reference-workload/first-room-fallback}")
SCORER_RESOLUTION="${M6_SCORER_RESOLUTION:-$HOME/git/preestablished/state-scorer/.agents/requests/phase4-m1-m4-first-boss-scoring/04-resolution.md}"
pass=0; fail=0
report() { printf '%-8s %-28s %s\n' "$1" "$2" "$3"; case "$1" in PASS) pass=$((pass + 1));; *) fail=$((fail + 1));; esac; }
terminal() { [ "$1" = closed ] || [ "$1" = "done" ]; }
issue_closed() {
  local id=$1 json actual status
  json=$(bn show "$id" --json 2>/dev/null) || return 1
  actual=$(printf '%s' "$json" | jq -er '.id | strings' 2>/dev/null) || return 1
  status=$(printf '%s' "$json" | jq -er '.status | strings' 2>/dev/null) || return 1
  [ "$actual" = "$id" ] && terminal "$status"
}
if ! command -v bn >/dev/null 2>&1 || ! command -v jq >/dev/null 2>&1; then
  report UNKNOWN scorer-M3 "bn or jq unavailable"
  report UNKNOWN refwork-czi "bn or jq unavailable"
  report UNKNOWN refwork-20v "bn or jq unavailable"
else
  scorer=$(bn list --project state-scorer --archived --json -n 0 2>/dev/null || true)
  if bn project show state-scorer --json 2>/dev/null | jq -e '.name == "state-scorer"' >/dev/null 2>&1 && printf '%s' "$scorer" | jq -e 'type == "array"' >/dev/null 2>&1; then
    m3_count=$(printf '%s' "$scorer" | jq '[.[] | select(.id == "state-scorer-0gy")] | length' 2>/dev/null || printf x)
    if [ "$m3_count" = 1 ] && issue_closed state-scorer-0gy; then report PASS scorer-M3 "state-scorer-0gy terminal"
    elif [ "$m3_count" = 0 ] && [ -f "$SCORER_RESOLUTION" ] && grep -F state-scorer-0gy "$SCORER_RESOLUTION" | grep -qi closed; then report PASS scorer-M3 "documented historical tracker evidence marks state-scorer-0gy closed"
    else report UNKNOWN scorer-M3 "unique terminal M3 record unavailable"; fi
  else report UNKNOWN scorer-M3 "state-scorer project or JSON list unavailable"; fi
  for id in refwork-czi refwork-20v; do if issue_closed "$id"; then report PASS "$id" terminal; else report FAIL "$id" "missing, malformed, wrong ID, or nonterminal"; fi; done
fi
probe_corpus() { local p; for p in "$@"; do [ -s "$p/manifest.json" ] && [ -s "$p/captures/index.jsonl" ] && { echo "$p"; return 0; }; done; return 1; }
probe_session() { local p; for p in "$@"; do [ -s "$p/session.yaml" ] && [ -s "$p/interactive.padlog" ] && { echo "$p"; return 0; }; done; return 1; }
probe() { local p; for p in "$@"; do { [ -d "$p" ] && [ -n "$(ls -A "$p" 2>/dev/null)" ]; } || { [ -f "$p" ] && [ -s "$p" ]; } && { echo "$p"; return 0; }; done; return 1; }
branch=NONE
if loc=$(probe_corpus "${CORPUS_CANDIDATES[@]}"); then branch=full-corpus; elif loc=$(probe "${FALLBACK_CANDIDATES[@]}"); then branch=first-room-fallback; elif loc=$(probe_session "${RAW_SESSION_CANDIDATES[@]}"); then branch=raw-session; fi
if [ "$branch" = NONE ]; then report FAIL hand-play-artifact "branch=NONE (no candidate path exists)"; else report PASS hand-play-artifact "branch=$branch at $loc"; [ "$branch" = first-room-fallback ] && echo 'WARNING: first-room-fallback branch — Phase 4 exit gate 3 is NOT declarable.'; fi
echo '── info (not gating) ──'
if command -v bn >/dev/null 2>&1 && bn show rom-operator-bridge-l1w --json >/dev/null 2>&1; then echo 'info: rom-operator-bridge-l1w visible via bn'; else echo 'info: rom-operator-bridge-l1w unavailable'; fi
systemctl --user list-units 2>/dev/null | grep -i bridge || echo 'info: no user bridge unit visible — confirm stack up before items 4-5'
echo "── result: $pass passed, $fail not-passed ──"
[ "$fail" -eq 0 ]
