#!/usr/bin/env bash
# m6-discovery-analyze2.sh — plan-driven offset-discovery analysis over one
# or more ramdiff sessions.
#
# Context: .agents/plans/phase4-m6-discovery-01-processing/ package 03
# (KNOWN GAP 1). The discovery-01 capture used trajectory-style labels, not
# the controlled Run-A label set tools/m6-discovery-analyze.sh hard-requires,
# so this sibling tool takes a --plan file instead of hardcoded
# REQUIRED_LABELS. The original tool's Run-A contract stays documented and
# untouched; structure, helpers, and the report/emit-draft format here are
# adapted from tools/m6-discovery-analyze.sh (cited per that package).
#
# Deltas vs the original:
#   - `--plan <frame-plan.yaml>` declares the exclusion pair and, per
#     feature: name, width, emit type/semantics/discretize, check_exclusion,
#     and ordered filter clauses (changed|unchanged|inc|dec|value|delta —
#     the `ramdiff search` filter surface). Dumps are referenced as
#     `<session>:<frame>` and resolved by FRAME NUMBER from that session's
#     session.yaml (label text is opaque — operator labels stay private).
#   - Scratch per-feature sessions symlink dumps from MULTIPLE source
#     sessions (discovery-01 + derived-01 + any later derived dirs).
#   - `--self-test` fabricates a synthetic 2-dump planted-byte session and
#     plan in a scratch dir and runs it through the real ramdiff binary
#     (the original's authoring-time verification, made repeatable).
#
# Standing constraint (identical to the original): never print decoded RAM
# values or offsets to stdout. Detail (candidate offsets, hexdump context,
# decoded values) goes ONLY to the report file. Stdout gets per-feature
# candidate COUNTS and warnings.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# ── usage ────────────────────────────────────────────────────────────────

usage() {
  cat <<'EOF'
Usage: m6-discovery-analyze2.sh --plan <frame-plan.yaml> [--out <report-file>]
       m6-discovery-analyze2.sh --self-test

Runs plan-driven ramdiff offset-discovery analysis and writes a private
analysis report.

The plan file (YAML) declares:
  sessions:  name -> session directory (absolute paths; private file)
  exclusion: {a: <sess>:<frame>, b: <sess>:<frame>, widths: [u8, u16le]}
  features:  list of {name, width, check_exclusion, emit: {type, semantics,
             discretize}, filters: [{op, a, b} | {op, value|in|delta...}]}
  gaps:      list of {name, reason} — reported and skipped, never failed

Options:
  --plan <file>    Path to the private frame plan (required).
  --out <file>     Report path (default: <plan-dir>/analysis2-report.txt).
  --self-test      Run the synthetic planted-byte self-test and exit.
  -h, --help       Show this help and exit.

Never prints decoded RAM values or offsets to stdout; only per-feature
candidate counts and warnings. Full detail goes to the report file.
EOF
}

# ── python with yaml (pipeline prereq; venv fallback per package 01) ─────

PYBIN=""
for p in python3 "$HOME/.venvs/refwork/bin/python3"; do
  if command -v "$p" >/dev/null 2>&1 && "$p" -c 'import yaml' 2>/dev/null; then
    PYBIN="$p"
    break
  fi
done
if [[ -z "$PYBIN" ]]; then
  echo "m6-discovery-analyze2: no python3 with pyyaml found (package-01 prereq)" >&2
  exit 1
fi

# ── argument parsing ─────────────────────────────────────────────────────

plan_file=""
out_file=""
self_test=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --plan)
      [[ $# -ge 2 ]] || { echo "m6-discovery-analyze2: --plan requires an argument" >&2; exit 1; }
      plan_file="$2"
      shift 2
      ;;
    --out)
      [[ $# -ge 2 ]] || { echo "m6-discovery-analyze2: --out requires an argument" >&2; exit 1; }
      out_file="$2"
      shift 2
      ;;
    --self-test)
      self_test=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "m6-discovery-analyze2: unknown argument: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

# ── locate the ramdiff binary ────────────────────────────────────────────

RAMDIFF_BIN=()
if [[ -x "$REPO_ROOT/target/release/ramdiff" ]]; then
  RAMDIFF_BIN=("$REPO_ROOT/target/release/ramdiff")
else
  echo "m6-discovery-analyze2: target/release/ramdiff not found, falling back to cargo run" >&2
  RAMDIFF_BIN=(cargo run --quiet --locked -p ramdiff --manifest-path "$REPO_ROOT/Cargo.toml" --)
fi

ramdiff() {
  "${RAMDIFF_BIN[@]}" "$@"
}

# ── self-test: synthetic 2-dump planted-byte session ─────────────────────

if [[ "$self_test" == "1" ]]; then
  st="$(mktemp -d "${TMPDIR:-/tmp}/m6-analyze2-selftest.XXXXXX")"
  trap 'rm -rf "$st"' EXIT
  mkdir -p "$st/sess"
  # 131072-byte all-zero dump A; dump B identical except one planted byte.
  "$PYBIN" - "$st/sess" <<'PYEOF'
import sys
d = sys.argv[1]
a = bytearray(131072)
open(f"{d}/planted-a.bin", "wb").write(a)
b = bytearray(a)
b[0x1234] = 0x2A
open(f"{d}/planted-b.bin", "wb").write(b)
PYEOF
  cat > "$st/sess/session.yaml" <<'YEOF'
dumps:
- label: planted-a
  frame: 100
  file: planted-a.bin
  region: wram
- label: planted-b
  frame: 200
  file: planted-b.bin
  region: wram
YEOF
  cat > "$st/plan.yaml" <<PEOF
kind: m6-frame-plan
schema_version: 1
sessions:
  synth: $st/sess
exclusion:
  a: "synth:100"
  b: "synth:100"
  widths: [u8]
features:
  - name: planted
    width: u8
    check_exclusion: true
    emit: {type: u8, semantics: mode, discretize: identity}
    filters:
      - {op: changed, a: "synth:100", b: "synth:200"}
gaps: []
PEOF
  out="$("$0" --plan "$st/plan.yaml" --out "$st/report.txt")"
  echo "$out"
  echo "$out" | grep -q '^planted: 1 candidate' \
    || { echo "SELF-TEST FAIL: expected exactly 1 planted candidate" >&2; exit 1; }
  grep -q '0x1234' "$st/report.txt" \
    || { echo "SELF-TEST FAIL: planted offset missing from report" >&2; exit 1; }
  echo "m6-discovery-analyze2: SELF-TEST PASS (1 candidate at the planted offset)"
  exit 0
fi

if [[ -z "$plan_file" ]]; then
  echo "m6-discovery-analyze2: --plan <file> is required" >&2
  usage >&2
  exit 1
fi
if [[ ! -f "$plan_file" ]]; then
  echo "m6-discovery-analyze2: plan file not found: $plan_file" >&2
  exit 1
fi
plan_file="$(cd "$(dirname "$plan_file")" && pwd)/$(basename "$plan_file")"

if [[ -z "$out_file" ]]; then
  out_file="$(dirname "$plan_file")/analysis2-report.txt"
fi

# ── flatten the plan to a line protocol (tab-separated) ──────────────────
#   SESSION <name> <path>
#   EXCLUSION <widths-csv> <refA> <refB>
#   FEATURE <name> <width> <0|1> <emit_type> <emit_semantics> <emit_discretize>
#   FILTER <feature> <op> <argA> [argB]
#   GAP <name> <reason>

plan_lines="$("$PYBIN" - "$plan_file" <<'PYEOF'
import sys, yaml
p = yaml.safe_load(open(sys.argv[1]))
if p.get("kind") != "m6-frame-plan" or p.get("schema_version") != 1:
    sys.exit("plan: kind/schema_version mismatch (want m6-frame-plan v1)")
for name, path in p["sessions"].items():
    print(f"SESSION\t{name}\t{path}")
ex = p["exclusion"]
print(f"EXCLUSION\t{','.join(ex['widths'])}\t{ex['a']}\t{ex['b']}")
for f in p["features"]:
    e = f["emit"]
    ce = 1 if f.get("check_exclusion") else 0
    print(f"FEATURE\t{f['name']}\t{f['width']}\t{ce}\t{e['type']}\t{e['semantics']}\t{e['discretize']}")
    for c in f["filters"]:
        op = c["op"]
        if op in ("changed", "unchanged", "inc", "dec", "delta"):
            optag = f"delta:{c['value']}" if op == "delta" else op
            print(f"FILTER\t{f['name']}\t{optag}\t{c['a']}\t{c['b']}")
        elif op in ("value", "in"):
            print(f"FILTER\t{f['name']}\t{op}\t{c['value']}")
        else:
            sys.exit(f"plan: unknown filter op {op!r} for {f['name']}")
for g in p.get("gaps") or []:
    reason = " ".join(str(g["reason"]).split())
    print(f"GAP\t{g['name']}\t{reason}")
PYEOF
)"

# ── plan tables ──────────────────────────────────────────────────────────

declare -A session_path=()
excl_widths=""
excl_a=""
excl_b=""

while IFS=$'\t' read -r kind f1 f2 f3 _rest; do
  case "$kind" in
    SESSION) session_path["$f1"]="$f2" ;;
    EXCLUSION) excl_widths="$f1"; excl_a="$f2"; excl_b="$f3" ;;
  esac
done <<< "$plan_lines"

for name in "${!session_path[@]}"; do
  d="${session_path[$name]}"
  if [[ ! -f "$d/session.yaml" ]]; then
    echo "m6-discovery-analyze2: session '$name' has no session.yaml: $d" >&2
    exit 1
  fi
done

# ── session.yaml helpers (adapted from tools/m6-discovery-analyze.sh) ────

# Look up the dump file registered for a given FRAME in a session.yaml.
lookup_dump_file_by_frame() {
  local frame="$1" yaml="$2"
  awk -v want="$frame" '
    /^- label: / { inblock = 1; f = ""; next }
    inblock && /^  frame: / {
      fr = $0
      sub(/^  frame: /, "", fr)
      match_frame = (fr == want)
      next
    }
    inblock && /^  file: / {
      if (match_frame) {
        f = $0
        sub(/^  file: /, "", f)
        gsub(/^"|"$/, "", f)
        print f
        exit
      }
    }
  ' "$yaml"
}

# Extract surviving candidate offsets (decimal, one per line) from a
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

# Resolve a <session>:<frame> ref -> "abs-dump-path scratch-label".
# Scratch label is <session>-<frame> ([A-Za-z0-9_-], collision-free).
resolve_ref() {
  local ref="$1"
  local sess="${ref%%:*}" frame="${ref##*:}"
  local dir="${session_path[$sess]:-}"
  if [[ -z "$dir" ]]; then
    echo "m6-discovery-analyze2: ref '$ref' names unknown session '$sess'" >&2
    return 1
  fi
  local file
  file="$(lookup_dump_file_by_frame "$frame" "$dir/session.yaml")"
  if [[ -z "$file" ]] || [[ ! -f "$dir/$file" ]]; then
    echo "m6-discovery-analyze2: no dump at frame $frame in session '$sess'" >&2
    return 1
  fi
  printf '%s\t%s\n' "$dir/$file" "${sess}-${frame}"
}

# ── scratch workspace: one isolated dir per feature ──────────────────────
# ramdiff's candidate set persists per session.yaml and intersects across
# `search` invocations against the SAME session.yaml; each feature gets its
# own scratch session (only the dumps it needs, candidates empty), with
# dump files symlinked from their SOURCE sessions (possibly several).

workdir="$(mktemp -d "${TMPDIR:-/tmp}/m6-discovery2.XXXXXX")"
cleanup() { rm -rf "$workdir"; }
trap cleanup EXIT

# new_feature_session <feature> <ref>... — builds the scratch session from
# <session>:<frame> refs; prints the scratch dir.
new_feature_session() {
  local feature="$1"
  shift
  local fdir="$workdir/$feature"
  mkdir -p "$fdir"
  local seen=" "
  {
    echo "dumps:"
    local ref line path label fname
    for ref in "$@"; do
      line="$(resolve_ref "$ref")" || exit 1
      path="${line%%$'\t'*}"
      label="${line##*$'\t'}"
      case "$seen" in *" $label "*) continue ;; esac
      seen="$seen$label "
      fname="$label.bin"
      printf -- '- label: %s\n  frame: 0\n  file: %s\n  region: wram\n' "$label" "$fname"
      ln -s "$path" "$fdir/$fname"
    done
  } > "$fdir/session.yaml"
  echo "$fdir"
}

# ref -> scratch label (must match new_feature_session's naming)
ref_label() {
  local ref="$1"
  echo "${ref%%:*}-${ref##*:}"
}

# ── report scaffolding ───────────────────────────────────────────────────

mkdir -p "$(dirname "$out_file")"
{
  echo "# M6 discovery analysis report (plan-driven)"
  echo "# Plan: $plan_file"
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

# ── stability-sanity exclusion set (plan's idle pair) ────────────────────

report "## Stability sanity: exclusion set ($excl_a vs $excl_b, idle)"
report ""

declare -A exclusion_offsets_u8=()
declare -A exclusion_offsets_u16=()

IFS=',' read -r -a widths_arr <<< "$excl_widths"
la="$(ref_label "$excl_a")"
lb="$(ref_label "$excl_b")"
for width in "${widths_arr[@]}"; do
  fdir="$(new_feature_session "exclusion-$width" "$excl_a" "$excl_b")"
  out_text="$(ramdiff search --session "$fdir" --width "$width" --changed "$la" "$lb" 2>&1 1>/dev/null)"
  count="$(parse_count "$out_text")"
  count="${count:-0}"
  report "- width $width: $count offset(s) changed while idle (volatile noise)"
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
# in the exclusion set for that width.
warn_exclusion_overlap() {
  local feature="$1" width="$2" fdir="$3"
  local -n excl_ref="exclusion_offsets_${width/le/}"
  local overlap=0
  while IFS= read -r off; do
    [[ -n "$off" ]] || continue
    if [[ -n "${excl_ref[$off]:-}" ]]; then
      overlap=$((overlap + 1))
      report "  WARNING: $feature candidate offset $off also appears in the" \
             "$width stability-sanity exclusion set (changed while idle)" \
             "— treat as suspect, re-verify before marking stable."
    fi
  done < <(extract_offsets "$fdir/session.yaml")
  if [[ "$overlap" -gt 0 ]]; then
    echo "  WARNING: $feature has $overlap candidate(s) overlapping the volatile-noise exclusion set (see report)"
  fi
  return 0
}

# ── per-feature analysis, driven by the plan ─────────────────────────────

report "## Plan-driven features"
report ""

feature_names="$(printf '%s\n' "$plan_lines" | awk -F'\t' '$1=="FEATURE"{print $2}')"

candidate_line_count=0

while IFS= read -r feature; do
  [[ -n "$feature" ]] || continue
  IFS=$'\t' read -r _ _ width check_exclusion emit_type emit_semantics emit_discretize \
    <<< "$(printf '%s\n' "$plan_lines" | awk -F'\t' -v f="$feature" '$1=="FEATURE" && $2==f')"

  # Collect refs + build filter args for this feature.
  refs=()
  filter_args=()
  while IFS=$'\t' read -r _ _ op a b; do
    [[ -n "$op" ]] || continue
    case "$op" in
      changed|unchanged|inc|dec)
        refs+=("$a" "$b")
        filter_args+=("--$op" "$(ref_label "$a")" "$(ref_label "$b")")
        ;;
      delta:*)
        d="${op#delta:}"
        refs+=("$a" "$b")
        filter_args+=(--delta "$d" "$(ref_label "$a")" "$(ref_label "$b")")
        ;;
      value) filter_args+=(--value "$a") ;;
      in)    filter_args+=(--in "$a") ;;
    esac
  done <<< "$(printf '%s\n' "$plan_lines" | awk -F'\t' -v f="$feature" '$1=="FILTER" && $2==f')"

  report "## Feature: $feature"
  report ""
  report "Filters: width=$width ${filter_args[*]}"

  fdir="$(new_feature_session "$feature" "${refs[@]}")"

  out_text="$(ramdiff search --session "$fdir" --width "$width" "${filter_args[@]}" 2>&1 1>/dev/null)"
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
             "revisit the filter clauses or add narrower derived dumps.)"
      ;;
    1)
      off="$(extract_offsets "$fdir/session.yaml" | head -n1)"
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
      report "  Narrow further (additional derived dumps / tighter filters) before emitting."
      ;;
  esac
  report ""

  echo "$feature: $count candidate(s)"
  candidate_line_count=$((candidate_line_count + 1))
done <<< "$feature_names"

# ── recorded gaps (reported and skipped, never failed) ───────────────────

while IFS=$'\t' read -r _ gname greason; do
  [[ -n "$gname" ]] || continue
  report "## Feature: $gname"
  report ""
  report "SKIPPED — $greason"
  report ""
  echo "$gname: skipped (recorded gap)"
done <<< "$(printf '%s\n' "$plan_lines" | awk -F'\t' '$1=="GAP"')"

echo "m6-discovery-analyze2: $candidate_line_count feature(s) analyzed" >&2
# Unlike the original tool, the report path is NOT echoed: the default report
# location is under the private plan dir, and this tool's stdout/stderr must
# stay free of private path components (plan privacy conventions).
echo "m6-discovery-analyze2: report written (path as given via --plan/--out)" >&2
