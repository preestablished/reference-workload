#!/usr/bin/env bash
# m6-discovery-analyze.sh — agent-side offset-discovery analysis for a
# completed `ramdiff record --interactive` session.
#
# Context: close-m6-entry-gates plan, package 03
# (.agents/plans/close-m6-entry-gates/03-close-refwork-20v.md), which points
# at the runbook
# .agents/plans/phase4-real-capture-corpus-fast-follow/03-private-map-layout-and-scoring.md
# step 2 ("discover offsets ... controlled hand-play"). The operator produces
# a session directory with F5 WRAM dumps under these exact labels:
#
#   baseline start-a1 start-a2 start-b room2 back-room1 area1
#   health-full health-hit pre-upgrade post-upgrade dead
#
# This script drives `ramdiff search`/`candidates` (see crates/ramdiff) over
# that session to narrow candidate offsets for each demo-map feature
# (feature-maps/demo-game.yaml: room_id, area_id, player_x, player_y,
# health, upgrade_flags, game_mode — boss_flags/credits_flag are NOT
# discoverable from this label set and are skipped, not failed).
#
# Everything downstream of the operator's `record --interactive` capture is
# non-interactive per the runbook's division of labor, so this script is
# fully agent-run: no ROM, no hand input.
#
# Standing constraint: never print decoded RAM values to stdout. Detail
# (candidate offsets, hexdump context, decoded values) goes ONLY to the
# report file. Stdout gets per-feature candidate COUNTS and warnings.
#
# Verification performed at authoring time (see plan package 03 report for
# detail): `bash -n` syntax check; a hand-fabricated synthetic session
# (2-dump, planted byte) run through the real `target/release/ramdiff`
# binary to confirm the on-disk session.yaml format (block-style YAML,
# `dumps:` list with label/frame/file/region, `candidates: {width, offsets}`
# with `- N` list items) matches what this script parses, and that
# `ramdiff search`/`candidates` behave as documented (multiple filter flags
# in one invocation intersect; the candidate set persists and further
# narrows across invocations against the same session.yaml). End-to-end
# behavior against a real 12-label session has NOT been run (no ROM/session
# available in this environment) — that verification happens on the real
# operator session per the runbook.

set -euo pipefail

# ── constants ────────────────────────────────────────────────────────────

REQUIRED_LABELS=(
  baseline start-a1 start-a2 start-b room2 back-room1 area1
  health-full health-hit pre-upgrade post-upgrade dead
)

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# ── usage ────────────────────────────────────────────────────────────────

usage() {
  cat <<'EOF'
Usage: m6-discovery-analyze.sh --session <dir> [--out <report-file>]

Runs the agent-side ramdiff offset-discovery analysis for a completed
`ramdiff record --interactive` session and writes a private analysis
report.

Required session dump labels (F5-labeled WRAM dumps):
  baseline start-a1 start-a2 start-b room2 back-room1 area1
  health-full health-hit pre-upgrade post-upgrade dead

Options:
  --session <dir>   Path to the ramdiff session directory (required).
  --out <file>      Report output path (default: <session>/analysis-report.txt).
  -h, --help        Show this help and exit.

Never prints decoded RAM values or offsets to stdout; only per-feature
candidate counts and warnings. Full detail goes to the report file.
EOF
}

# ── argument parsing ─────────────────────────────────────────────────────

session_dir=""
out_file=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --session)
      [[ $# -ge 2 ]] || { echo "m6-discovery-analyze: --session requires an argument" >&2; exit 1; }
      session_dir="$2"
      shift 2
      ;;
    --out)
      [[ $# -ge 2 ]] || { echo "m6-discovery-analyze: --out requires an argument" >&2; exit 1; }
      out_file="$2"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "m6-discovery-analyze: unknown argument: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

if [[ -z "$session_dir" ]]; then
  echo "m6-discovery-analyze: --session <dir> is required" >&2
  usage >&2
  exit 1
fi

if [[ ! -d "$session_dir" ]]; then
  echo "m6-discovery-analyze: session directory not found: $session_dir" >&2
  exit 1
fi

session_dir="$(cd "$session_dir" && pwd)"

session_yaml="$session_dir/session.yaml"
if [[ ! -f "$session_yaml" ]]; then
  echo "m6-discovery-analyze: no session.yaml in $session_dir (not a ramdiff session dir?)" >&2
  exit 1
fi

if [[ -z "$out_file" ]]; then
  out_file="$session_dir/analysis-report.txt"
fi

# ── locate the ramdiff binary ────────────────────────────────────────────

RAMDIFF_BIN=()
if [[ -x "$REPO_ROOT/target/release/ramdiff" ]]; then
  RAMDIFF_BIN=("$REPO_ROOT/target/release/ramdiff")
else
  echo "m6-discovery-analyze: target/release/ramdiff not found, falling back to cargo run" >&2
  RAMDIFF_BIN=(cargo run --quiet --locked -p ramdiff --manifest-path "$REPO_ROOT/Cargo.toml" --)
fi

ramdiff() {
  "${RAMDIFF_BIN[@]}" "$@"
}

# ── session.yaml parsing helpers ─────────────────────────────────────────

# Look up the dump file name registered for a given label in a session.yaml.
# Prints the file name (relative to the session dir) or nothing if absent.
lookup_dump_file() {
  local label="$1" yaml="$2"
  awk -v want="$label" '
    /^- label: / {
      cur = $0
      sub(/^- label: /, "", cur)
      gsub(/^"|"$/, "", cur)
      active = (cur == want)
      next
    }
    active && /^  file: / {
      f = $0
      sub(/^  file: /, "", f)
      gsub(/^"|"$/, "", f)
      print f
      exit
    }
  ' "$yaml"
}

# Extract the surviving candidate offsets (decimal, one per line) from a
# feature working session.yaml. Offsets only — never decoded values.
extract_offsets() {
  local yaml="$1"
  awk '
    /^candidates:/ { incand = 1; next }
    incand && /^  offsets:/ { inoff = 1; next }
    inoff && /^  - [0-9]+/ { print $2; next }
    inoff && !/^  - / { inoff = 0 }
  ' "$yaml"
}

# Parse the trailing "search: N candidate(s) remain" count off stderr text.
parse_count() {
  local text="$1"
  echo "$text" | grep -oE 'search: [0-9]+ candidate' | grep -oE '[0-9]+' | tail -n1
}

# ── validate required labels are present ─────────────────────────────────

missing_labels=()
declare -A dump_file_for
for label in "${REQUIRED_LABELS[@]}"; do
  file="$(lookup_dump_file "$label" "$session_yaml" || true)"
  if [[ -z "$file" ]] || [[ ! -f "$session_dir/$file" ]]; then
    missing_labels+=("$label")
  else
    dump_file_for["$label"]="$file"
  fi
done

if [[ ${#missing_labels[@]} -gt 0 ]]; then
  {
    echo "m6-discovery-analyze: session is missing ${#missing_labels[@]} required dump label(s):"
    for m in "${missing_labels[@]}"; do
      echo "  - $m"
    done
    echo "All labels required: ${REQUIRED_LABELS[*]}"
  } >&2
  exit 1
fi

# ── scratch workspace: one isolated feature dir per feature ──────────────
# ramdiff's candidate set is persisted per session.yaml and accumulates
# (intersects) across `search` invocations against the SAME session.yaml.
# To keep features from narrowing each other's candidate sets, each feature
# gets its own scratch session.yaml (only the dump labels it needs,
# candidates empty) with the real dump files symlinked in.

workdir="$(mktemp -d "${TMPDIR:-/tmp}/m6-discovery.XXXXXX")"
cleanup() { rm -rf "$workdir"; }
trap cleanup EXIT

new_feature_session() {
  local feature="$1"
  shift
  local labels=("$@")
  local fdir="$workdir/$feature"
  mkdir -p "$fdir"
  {
    echo "dumps:"
    for lbl in "${labels[@]}"; do
      local file="${dump_file_for[$lbl]}"
      printf -- '- label: %s\n  frame: 0\n  file: %s\n  region: wram\n' "$lbl" "$file"
      ln -s "$session_dir/$file" "$fdir/$file"
    done
  } > "$fdir/session.yaml"
  echo "$fdir"
}

# ── report scaffolding ────────────────────────────────────────────────────

mkdir -p "$(dirname "$out_file")"
{
  echo "# M6 discovery analysis report"
  echo "# Session: $session_dir"
  echo "# Generated: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "#"
  echo "# Private working evidence only. Contains offsets and decoded WRAM"
  echo "# values for this operator's session/ROM. Do not commit; do not paste"
  echo "# into public records, beads, or terminal transcripts."
  echo
} > "$out_file"

report() {
  printf '%s\n' "$*" >> "$out_file"
}

# ── stability-sanity exclusion set (start-a1 vs start-a2, standing still) ─
# Anything that differs here while the operator held still is volatile
# background noise (frame counters, RNG, audio/timer ticks, ...), not a
# candidate for any feature this script marks "stable".

report "## Stability sanity: exclusion set (start-a1 vs start-a2, standing still)"
report ""

declare -A exclusion_offsets_u8=()
declare -A exclusion_offsets_u16=()

for width in u8 u16le; do
  fdir="$(new_feature_session "exclusion-$width" start-a1 start-a2)"
  out_text="$(ramdiff search --session "$fdir" --width "$width" --changed start-a1 start-a2 2>&1 1>/dev/null)"
  count="$(parse_count "$out_text")"
  count="${count:-0}"
  report "- width $width: $count offset(s) changed while standing still (volatile noise)"
  while IFS= read -r off; do
    [[ -n "$off" ]] || continue
    if [[ "$width" == "u8" ]]; then
      exclusion_offsets_u8["$off"]=1
    else
      exclusion_offsets_u16["$off"]=1
    fi
  done < <(extract_offsets "$fdir/session.yaml")
  echo "exclusion-set[$width]: $count offset(s)"
done

report ""

# Warn if any offset in a "stable"-expected feature's surviving set is also
# in the exclusion set for that width. Returns 0 (no overlap) or 1 (overlap).
warn_exclusion_overlap() {
  local feature="$1" width="$2" fdir="$3"
  local -n excl_ref="exclusion_offsets_${width/le/}"
  local overlap=0
  while IFS= read -r off; do
    [[ -n "$off" ]] || continue
    if [[ -n "${excl_ref[$off]:-}" ]]; then
      overlap=$((overlap + 1))
      report "  WARNING: $feature candidate offset $off also appears in the" \
             "$width stability-sanity exclusion set (changed while standing" \
             "still) — treat as suspect, re-verify before marking stable."
    fi
  done < <(extract_offsets "$fdir/session.yaml")
  if [[ "$overlap" -gt 0 ]]; then
    echo "  WARNING: $feature has $overlap candidate(s) overlapping the volatile-noise exclusion set (see report)"
  fi
  return 0
}

# ── generic per-feature analysis ──────────────────────────────────────────
# Runs one `ramdiff search` invocation with all the given filter flags
# (multiple filter flags in a single invocation intersect — confirmed
# against crates/ramdiff/src/filter.rs run_search, which applies every op
# in `ops` against the same candidate set in one pass), then dumps
# candidates to the report and emits a draft `ramdiff emit` line.
#
# Args: feature width check_exclusion(0|1) emit_type emit_semantics
#       emit_discretize labels_used... -- filter_args...
run_feature() {
  local feature="$1" width="$2" check_exclusion="$3"
  local emit_type="$4" emit_semantics="$5" emit_discretize="$6"
  shift 6

  local labels=()
  while [[ "$1" != "--" ]]; do
    labels+=("$1")
    shift
  done
  shift # drop --
  local filter_args=("$@")

  report "## Feature: $feature"
  report ""
  report "Filters: width=$width ${filter_args[*]}"

  local fdir
  fdir="$(new_feature_session "$feature" "${labels[@]}")"

  local out_text
  out_text="$(ramdiff search --session "$fdir" --width "$width" "${filter_args[@]}" 2>&1 1>/dev/null)"
  local count
  count="$(parse_count "$out_text")"
  count="${count:-0}"

  report "Candidates surviving: $count"
  report ""

  if [[ "$check_exclusion" == "1" ]]; then
    warn_exclusion_overlap "$feature" "$width" "$fdir"
  fi

  if [[ "$count" -gt 0 ]]; then
    report "### Candidate detail (offset, per-dump decoded value, hexdump context)"
    report ""
    ramdiff candidates --session "$fdir" --limit 25 --context 4 >> "$out_file" 2>&1 || true
  fi

  report "### Draft emit command"
  report ""
  case "$count" in
    0)
      report "  (no surviving candidates — discovery inconclusive for '$feature';" \
             "revisit the state-change script for this feature, or widen the" \
             "comparison dumps.)"
      ;;
    1)
      local off
      off="$(extract_offsets "$fdir/session.yaml" | head -n1)"
      local off_hex
      off_hex="$(printf '0x%04X' "$off")"
      report "  ${RAMDIFF_BIN[*]} emit --map <PRIVATE_MAP.yaml> \\"
      report "    --name $feature --offset $off_hex --type $emit_type \\"
      report "    --stability <stable|volatile> --semantics $emit_semantics \\"
      report "    --discretize $emit_discretize --description \"TODO: describe $feature\""
      if [[ "$emit_discretize" != "identity" && "$emit_discretize" != "none" && "$emit_discretize" != "bits" ]]; then
        report "  NOTE: 'ramdiff emit' only accepts --discretize identity|none|bits;" \
               "a $emit_discretize discretization (grid/threshold-style) must be" \
               "hand-edited into the map YAML after emit, per feature-maps/demo-game.yaml."
      fi
      ;;
    *)
      report "  Ambiguous: $count candidates survived, offsets (hex):"
      while IFS= read -r off; do
        [[ -n "$off" ]] || continue
        report "    - $(printf '0x%04X' "$off")"
      done < <(extract_offsets "$fdir/session.yaml")
      report "  Narrow further (additional dumps / tighter filters) before emitting."
      ;;
  esac
  report ""

  echo "$feature: $count candidate(s)"
}

# ── feature discovery plan ────────────────────────────────────────────────

report "## Discoverable features (from feature-maps/demo-game.yaml structure)"
report ""

# room_id (u16le, stable): same room across start-a1/start-b (only position
# moved), changed on entering room2, and back to the start-a1 value on
# return (expressed as unchanged(start-a1, back-room1), since the script
# does not know the actual room_id value ahead of discovery).
run_feature room_id u16le 1 u16le room_id identity \
  start-a1 start-b room2 back-room1 -- \
  --unchanged start-a1 start-b --changed start-a1 room2 --unchanged start-a1 back-room1

# area_id (u8, stable): unchanged across start-a1/start-b/room2 (same zone),
# changed once the operator crosses into a different macro area.
run_feature area_id u8 1 u8 room_id identity \
  start-a1 start-b room2 area1 -- \
  --unchanged start-a1 start-b --unchanged start-a1 room2 --changed start-a1 area1

# player_x / player_y (u16le, volatile): changed while moving within the
# same room (start-a1 -> start-b), unchanged while standing still
# (start-a1 -> start-a2) to cut jitter/noise. NOTE: with only these labels
# the x and y channel cannot be told apart from search alone — both share
# the same discriminating dump pair. Confirm the axis via candidate
# hexdump context (adjacent bytes, expected value ranges) or capture an
# axis-isolated dump pair (move only horizontally / only vertically).
run_feature player_x u16le 0 u16le position_x grid \
  start-a1 start-a2 start-b -- \
  --unchanged start-a1 start-a2 --changed start-a1 start-b
run_feature player_y u16le 0 u16le position_y none \
  start-a1 start-a2 start-b -- \
  --unchanged start-a1 start-a2 --changed start-a1 start-b
report "  NOTE: player_x and player_y are discovered with the identical" \
       "filter pair above (no axis-isolated dumps in this label set) —" \
       "both candidate lists are the same set; disambiguate manually via" \
       "hexdump context or a follow-up axis-isolated capture."
report ""

# health (u16le, stable classification per demo map despite changing on
# hit): strictly decreases from health-full to health-hit.
run_feature health u16le 1 u16le health threshold \
  health-full health-hit -- \
  --dec health-full health-hit

# upgrade_flags (bitflags16le, stable): changes once the first upgrade is
# picked up. Only "changed" is filtered here (not "increased") because a
# stronger heuristic risks excluding the true candidate if the bit layout
# doesn't monotonically increase; manually confirm in the report detail
# that the pre->post delta is a single set bit (a power of two), matching
# "first upgrade" expectations (bit 0 per feature-maps/demo-game.yaml).
run_feature upgrade_flags u16le 1 bitflags16le flags bits \
  pre-upgrade post-upgrade -- \
  --changed pre-upgrade post-upgrade
report "  NOTE: verify the pre-upgrade -> post-upgrade delta for the chosen" \
       "candidate is a single bit (value is a power of two); if not, this" \
       "may not be a clean bitflags field or more than one upgrade landed" \
       "in the capture."
report ""

# game_mode (u8, stable): distinguishes gameplay from menu/baseline and
# from the death state.
run_feature game_mode u8 1 u8 mode identity \
  baseline start-a1 dead -- \
  --changed baseline start-a1 --changed start-a1 dead

# boss_flags / credits_flag: NOT discoverable from this label set (no
# boss-defeat or credits/late-game dumps in Run A). Degrade gracefully:
# warn and skip rather than fail.
report "## Feature: boss_flags"
report ""
report "SKIPPED — not discoverable from this session's label set. The demo" \
       "map's boss_flags bitmask only changes on a boss defeat, which this" \
       "Run A session does not capture (see plan package 03/04: boss" \
       "segments and credits/late-game fixtures come later). Revisit with a" \
       "session that includes pre-boss/post-boss dump labels."
report ""
echo "boss_flags: skipped (not discoverable from Run A)"

report "## Feature: credits_flag"
report ""
report "SKIPPED — not discoverable from this session's label set (no" \
       "credits/late-game dump). Revisit with a session that includes a" \
       "pre-credits/post-credits (or equivalent late-game) dump pair."
report ""
echo "credits_flag: skipped (not discoverable from Run A)"

echo "m6-discovery-analyze: report written to $out_file" >&2
