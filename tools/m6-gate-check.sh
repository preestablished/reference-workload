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
if [ -d "$SCORER_REPO/.beads" ] && command -v bd >/dev/null; then
  m3_line=$( (cd "$SCORER_REPO" && bd list --limit 0 2>/dev/null) | grep -i 'M3' || true)
  if [ -n "$m3_line" ] && echo "$m3_line" | grep -q '✓'; then
    report PASS "scorer-M3" "$m3_line"
  else
    report FAIL "scorer-M3" "${m3_line:-no M3 bead found in state-scorer}"
  fi
else
  report UNKNOWN "scorer-M3" "verify in state-scorer packet (no beads DB found) — counts as not-passed"
fi

# 2–3. this repo's beads
for b in refwork-czi refwork-20v; do
  if bead_closed "$b"; then
    report PASS "$b" "closed"
  else
    report FAIL "$b" "$(bd show "$b" 2>/dev/null | head -1 || echo 'bd unavailable')"
  fi
done

# 4. hand-play artifact — probe in preference order; report which branch.
branch=NONE
probe() { for p in "$@"; do [ -e "$p" ] && { echo "$p"; return 0; }; done; return 1; }
if loc=$(probe "${CORPUS_CANDIDATES[@]}"); then
  branch=full-corpus
elif loc=$(probe "${FALLBACK_CANDIDATES[@]}"); then
  branch=first-room-fallback
elif loc=$(probe "${RAW_SESSION_CANDIDATES[@]}"); then
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
bd show rom-operator-bridge-l1w >/dev/null 2>&1 \
  && echo "info: rom-operator-bridge-l1w visible via bd (check its state for smoke-window coordination)" \
  || echo "info: hypervisor leak bead rom-operator-bridge-l1w — check rom-operator-bridge repo before scheduling the smoke"
systemctl --user list-units 2>/dev/null | grep -i bridge || echo "info: no user bridge unit visible — confirm stack up before items 4-5"

echo "── result: $pass passed, $fail not-passed ──"
[ "$fail" -eq 0 ]
