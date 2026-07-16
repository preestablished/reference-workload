#!/usr/bin/env bash
# M6 entry-gate checker — .agents/requests/phase4-m6-scoring-goal-integration/
# Prints PASS/FAIL/UNKNOWN per entry condition; exits 0 only if all four pass.
# UNKNOWN counts as not-passed (fail closed).
set -u

# ── CANDIDATE PATHS ─────────────────────────────────────────────────────────
# Update these when the fast-follow freezes real locations (its packages
# 04/04a/05 define them). Stale paths must fail closed, never pass.
CORPUS_CANDIDATES=(
  "$HOME/.agents/projects/reference-workload/corpus"
  "$(dirname "$0")/../data/corpus"
)
RAW_SESSION_CANDIDATES=(
  "$HOME/.agents/projects/reference-workload/handplay-session"
)
FALLBACK_CANDIDATES=(
  "$HOME/.agents/projects/reference-workload/first-room-fallback"
)
SCORER_REPO="$HOME/git/preestablished/state-scorer"
# ────────────────────────────────────────────────────────────────────────────

pass=0
fail=0

report() { # status label detail
  printf '%-8s %-28s %s\n' "$1" "$2" "$3"
  case "$1" in PASS) pass=$((pass+1));; *) fail=$((fail+1));; esac
}

bead_closed() {
  bd show "$1" 2>/dev/null | head -1 | grep -q 'CLOSED'
}

# 1. scorer M3 closed — lives in another repo; only PASS on positive evidence.
# Word-anchored match (-w) so IDs/titles merely containing "m3" as a substring
# (e.g. "arm32", a hash like "xm3q") can never fail-open; the closed marker
# must be at line start. Re-verify the naming convention once state-scorer
# actually creates its M1-M4 milestone beads (their packet's work item 0).
scorer_milestone() { # $1=M3|M4 -> echoes matching bd list line, if any
  # --all: closed beads are hidden by default, and closed is exactly the
  # state this gate is looking for.
  (cd "$SCORER_REPO" && bd list --limit 0 --all 2>/dev/null) | grep -iw "$1" || true
}
scorer_db_alive() {
  [ -d "$SCORER_REPO/.beads" ] && command -v bd >/dev/null \
    && (cd "$SCORER_REPO" && bd list --limit 1 >/dev/null 2>&1)
}
# Documented-evidence fallback: the m6-host teardown lost both repos' embedded
# Dolt DBs. When the scorer DB is absent, positive documented evidence — their
# filed resolution's beads table marking M3 closed — is accepted; anything
# less stays UNKNOWN/not-passed.
SCORER_RESOLUTION="$SCORER_REPO/.agents/requests/phase4-m1-m4-first-boss-scoring/04-resolution.md"
if scorer_db_alive; then
  m3_line=$(scorer_milestone M3)
  if [ -n "$m3_line" ] && printf '%s\n' "$m3_line" | grep -q '^✓'; then
    report PASS "scorer-M3" "$m3_line"
  elif [ -n "$m3_line" ]; then
    report FAIL "scorer-M3" "$m3_line"
  else
    report UNKNOWN "scorer-M3" "no M3-titled bead in state-scorer — verify in their packet; counts as not-passed"
  fi
elif [ -f "$SCORER_RESOLUTION" ] \
  && grep -F 'state-scorer-0gy' "$SCORER_RESOLUTION" | grep -qi 'closed'; then
  report PASS "scorer-M3" "documented evidence (scorer DB lost): $SCORER_RESOLUTION marks state-scorer-0gy (M3) closed"
else
  report UNKNOWN "scorer-M3" "verify in state-scorer packet (no beads DB, no documented closure evidence) — counts as not-passed"
fi

# 2–3. this repo's beads
for b in refwork-czi refwork-20v; do
  if bead_closed "$b"; then
    report PASS "$b" "closed"
  else
    detail=$(bd show "$b" 2>/dev/null | head -1)
    report FAIL "$b" "${detail:-bd unavailable or bead missing}"
  fi
done

# 4. hand-play artifact — probe in preference order; report which branch.
branch=NONE
# Marker files required, not bare non-empty directories: a stray `mkdir -p`
# or zero-byte file must not pass the gate.
#   corpus  : manifest.json AND captures/index.jsonl (both non-empty)
#   session : session.yaml AND interactive.padlog (both non-empty)
probe_corpus() {
  for p in "$@"; do
    if [ -s "$p/manifest.json" ] && [ -s "$p/captures/index.jsonl" ]; then
      echo "$p"; return 0
    fi
  done
  return 1
}
probe_session() {
  for p in "$@"; do
    if [ -s "$p/session.yaml" ] && [ -s "$p/interactive.padlog" ]; then
      echo "$p"; return 0
    fi
  done
  return 1
}
probe() {
  for p in "$@"; do
    if [ -d "$p" ] && [ -n "$(ls -A "$p" 2>/dev/null)" ]; then echo "$p"; return 0; fi
    if [ -f "$p" ] && [ -s "$p" ]; then echo "$p"; return 0; fi
  done
  return 1
}
if loc=$(probe_corpus "${CORPUS_CANDIDATES[@]}"); then
  branch=full-corpus
elif loc=$(probe "${FALLBACK_CANDIDATES[@]}"); then
  branch=first-room-fallback
elif loc=$(probe_session "${RAW_SESSION_CANDIDATES[@]}"); then
  branch=raw-session
fi
if [ "$branch" = NONE ]; then
  report FAIL "hand-play-artifact" "branch=NONE (no candidate path exists; update CANDIDATE PATHS if the fast-follow has frozen locations)"
else
  report PASS "hand-play-artifact" "branch=$branch at $loc"
  if [ "$branch" = first-room-fallback ]; then
    echo "WARNING: first-room-fallback branch — item 3 (gate-3 fixture) CANNOT run;"
    echo "         Phase 4 exit gate 3 is NOT declarable; record the reduction."
  fi
fi

# Informational (not gating)
echo "── info (not gating) ──"
if [ -d "$SCORER_REPO/.beads" ] && command -v bd >/dev/null; then
  m4_line=$(scorer_milestone M4)
  echo "info: scorer-M4 (items 4-5 need it): ${m4_line:-no M4-titled bead in state-scorer}"
else
  echo "info: scorer-M4 status unknown (no beads DB found in state-scorer)"
fi
bd show rom-operator-bridge-l1w >/dev/null 2>&1 \
  && echo "info: rom-operator-bridge-l1w visible via bd (check its state for smoke-window coordination)" \
  || echo "info: hypervisor leak bead rom-operator-bridge-l1w — check rom-operator-bridge repo before scheduling the smoke"
systemctl --user list-units 2>/dev/null | grep -i bridge || echo "info: no user bridge unit visible — confirm stack up before items 4-5"

echo "── result: $pass passed, $fail not-passed ──"
[ "$fail" -eq 0 ]
