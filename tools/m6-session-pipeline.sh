#!/usr/bin/env bash
# m6-session-pipeline.sh — session-day execution layer for closing the M6
# entry gates (refwork-20v real map/layout validation, refwork-5tk full
# corpus capture).
#
# Context: .agents/plans/close-m6-entry-gates/ packages 03-05, which point
# at the fast-follow runbooks
# .agents/plans/phase4-real-capture-corpus-fast-follow/03-private-map-layout-and-scoring.md
# and 04-operator-capture-and-labeling.md. Those runbooks describe a long
# chain of `cargo run -p refwork-featuremap` / `cargo run -p refwork-verify`
# invocations; this script turns that chain into one parameterized,
# resumable, idempotent pipeline so session day is mechanical.
#
# Standing constraints (see .agents/plans/close-m6-entry-gates/00-overview.md):
#   - never track private game-derived payloads; all bundle/report files
#     produced by this script live under --private-root, never under the
#     git checkout;
#   - no ROM names, offsets, decoded values, or private absolute paths in
#     stdout/stderr — every stage captures full command output ONLY into a
#     private log file under $BUNDLE/validation/, and prints just the stage
#     name, pass/fail, and the (private) report path to the terminal;
#   - a stage that would overwrite a non-empty prior output requires
#     --force.
#
# Subcommands (one per fast-follow step): validate-map, map-check, layout,
# capture, artifact-check, score-plan, trace, status. Run
# `m6-session-pipeline.sh <stage> --help` for stage-specific flags, or see
# usage() below.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

DEMO_MAP="$REPO_ROOT/feature-maps/demo-game.yaml"
DEFAULT_EXPORTER_COMMIT="2827665"
DEFAULT_ENDPOINT="unix:///run/dh/grpc.sock"
DEFAULT_REAL_ENV="$HOME/.rbo73/m4-regen-20260707/handoff/real.env"

# ── global usage ─────────────────────────────────────────────────────────

usage() {
  cat <<'EOF'
Usage: m6-session-pipeline.sh <stage> --private-root <dir> [stage flags]

Stages (run in this order; each is independently re-runnable):
  validate-map     featuremap validate of feature-map.yaml + scoring-program.yaml
  map-check        real map-check against the approved ROM/script/expectations
  layout           generate + independently review phase4-layout output
  capture          phase4-capture-export against the deployed worker
  artifact-check   phase4-artifact-check over the exported bundle
  score-plan       K=32 phase4-score-plan (first-boss/goal-positive/goal-negative)
  trace            trajectory + trace report for the first-boss trajectory
  status           show which stages have completed, resumably

Global flags (every stage):
  --private-root <dir>   Required. Bundle root is <dir>/bundle.
  --force                Allow overwriting a non-empty prior stage output.
  -h, --help              Show this usage, or run `<stage> --help` for
                          stage-specific flags.

Every stage validates its own inputs first and names the earlier stage that
produces any missing input. No file contents (offsets, decoded values, ROM
identity) are ever printed to stdout/stderr; per-stage detail goes only to
private log/report files under <private-root>/bundle/validation/.
EOF
}

if [[ $# -eq 0 ]]; then
  usage
  exit 1
fi

STAGE="$1"
shift

case "$STAGE" in
  -h|--help|help)
    usage
    exit 0
    ;;
  validate-map|map-check|layout|capture|artifact-check|score-plan|trace|status)
    ;;
  *)
    echo "m6-session-pipeline: unknown stage '$STAGE'" >&2
    usage >&2
    exit 1
    ;;
esac

# ── shared helpers ───────────────────────────────────────────────────────

PRIVATE_ROOT=""
FORCE=0
STAGE_ARGS=()

# First pass: peel off the global flags (--private-root/--force/-h) from
# anywhere in the arg list; stage-specific flags are collected in order
# into STAGE_ARGS for each stage's own parser.
while [[ $# -gt 0 ]]; do
  case "$1" in
    --private-root)
      [[ $# -ge 2 ]] || { echo "m6-session-pipeline: --private-root requires an argument" >&2; exit 1; }
      PRIVATE_ROOT="$2"
      shift 2
      ;;
    --force)
      FORCE=1
      shift
      ;;
    -h|--help)
      STAGE_ARGS+=("--help")
      shift
      ;;
    *)
      STAGE_ARGS+=("$1")
      shift
      ;;
  esac
done

if [[ "$STAGE" != "status" ]] || [[ ${#STAGE_ARGS[@]} -eq 0 || "${STAGE_ARGS[0]:-}" != "--help" ]]; then
  :
fi

if [[ -z "$PRIVATE_ROOT" ]]; then
  echo "m6-session-pipeline: --private-root <dir> is required" >&2
  exit 1
fi

# Resolve to an absolute path without requiring it to exist yet (status/
# early stages may run before the bundle dir is created).
mkdir -p "$PRIVATE_ROOT"
PRIVATE_ROOT="$(cd "$PRIVATE_ROOT" && pwd)"
BUNDLE="$PRIVATE_ROOT/bundle"
VALIDATION_DIR="$BUNDLE/validation"
mkdir -p "$VALIDATION_DIR"

die() {
  echo "m6-session-pipeline[$STAGE]: $*" >&2
  exit 1
}

need_input() {
  # need_input <path> <description> <producing-stage-or-source>
  local path="$1" desc="$2" source="$3"
  if [[ ! -e "$path" ]]; then
    die "missing input: $desc not found at the expected path. Produced by: $source"
  fi
}

# check_overwrite <path> — refuse to clobber a non-empty prior output
# unless --force was passed.
check_overwrite() {
  local path="$1"
  if [[ "$FORCE" -eq 1 ]]; then
    return 0
  fi
  if [[ -f "$path" ]] && [[ -s "$path" ]]; then
    die "refusing to overwrite existing non-empty output '$path' (pass --force to overwrite)"
  fi
  if [[ -d "$path" ]] && [[ -n "$(ls -A "$path" 2>/dev/null)" ]]; then
    die "refusing to overwrite existing non-empty output directory '$path' (pass --force to overwrite)"
  fi
}

# resolve_bin <crate> <bin-name> — prefer a release build, else `cargo run`.
# Sets the global array BIN_CMD.
resolve_bin() {
  local crate="$1" bin="$2"
  if [[ -x "$REPO_ROOT/target/release/$bin" ]]; then
    BIN_CMD=("$REPO_ROOT/target/release/$bin")
  else
    BIN_CMD=(cargo run --quiet --locked -p "$crate" --bin "$bin" --manifest-path "$REPO_ROOT/Cargo.toml" --)
  fi
}

VERIFY_BIN=()
resolve_verify_bin() { resolve_bin refwork-verify refwork-verify; VERIFY_BIN=("${BIN_CMD[@]}"); }

FEATUREMAP_BIN=()
resolve_featuremap_bin() { resolve_bin refwork-featuremap refwork-featuremap; FEATUREMAP_BIN=("${BIN_CMD[@]}"); }

now_iso() { date -u +%Y-%m-%dT%H:%M:%SZ; }

# run_wrapped <report-json> <log-path> <cmd...>
#
# Runs a command whose native output has no durable machine-readable report
# mode (validate-map, map-check, layout generation stdout, score-plan
# stdout). Captures combined stdout+stderr ONLY into the private log file
# (never to this script's stdout — command output may contain private
# offsets/decoded values/paths) and writes a small wrapper JSON report with
# exit code + pass/fail + a pointer to the log. Returns the command's exit
# code.
run_wrapped() {
  local report_json="$1" log_path="$2"
  shift 2
  local rc=0
  if "$@" >"$log_path" 2>&1; then
    rc=0
  else
    rc=$?
  fi
  local status="fail"
  [[ "$rc" -eq 0 ]] && status="pass"
  jq -n \
    --arg stage "$STAGE" \
    --arg status "$status" \
    --argjson exit_code "$rc" \
    --arg generated_at "$(now_iso)" \
    --arg log "$log_path" \
    '{stage:$stage, status:$status, exit_code:$exit_code, generated_at:$generated_at, log:$log}' \
    > "$report_json"
  return "$rc"
}

# report_status <report-json-path> — prints "PASS"/"FAIL"/"MISSING" by
# reading a `.status` field common to every native refwork-verify Phase 4
# report struct and to this script's own wrapper reports.
report_status() {
  local path="$1"
  if [[ ! -s "$path" ]]; then
    echo "MISSING"
    return
  fi
  local s
  s="$(jq -r '.status // empty' "$path" 2>/dev/null || true)"
  case "$s" in
    pass) echo "PASS" ;;
    fail) echo "FAIL" ;;
    *) echo "UNREADABLE" ;;
  esac
}

print_result() {
  # print_result <pass|fail> <report-path>
  local ok="$1" report="$2"
  if [[ "$ok" == "pass" ]]; then
    echo "$STAGE: PASS — report=$report"
  else
    echo "$STAGE: FAIL — report=$report" >&2
  fi
}

# ── stage: validate-map ──────────────────────────────────────────────────

cmd_validate_map() {
  local map="$BUNDLE/feature-map.yaml"
  local scoring="$BUNDLE/scoring-program.yaml"
  local args=("${STAGE_ARGS[@]}")
  local i=0
  while [[ $i -lt ${#args[@]} ]]; do
    case "${args[$i]}" in
      --map) map="${args[$((i+1))]}"; i=$((i+2)) ;;
      --scoring) scoring="${args[$((i+1))]}"; i=$((i+2)) ;;
      --help)
        cat <<EOF
Usage: m6-session-pipeline.sh validate-map --private-root <dir> [--map <path>] [--scoring <path>]

Runs \`refwork-featuremap validate <map> --scoring <scoring>\` (fast-follow
03 step 3). Defaults: --map \$BUNDLE/feature-map.yaml,
--scoring \$BUNDLE/scoring-program.yaml.
EOF
        exit 0
        ;;
      *) die "unknown flag '${args[$i]}'" ;;
    esac
  done

  need_input "$map" "feature-map.yaml" "the map-authoring loop (SESSION-DAY-RUNBOOK.md); not produced by this pipeline"
  need_input "$scoring" "scoring-program.yaml" "the map-authoring loop (SESSION-DAY-RUNBOOK.md); not produced by this pipeline"

  local report="$VALIDATION_DIR/featuremap-validate.json"
  local log="$VALIDATION_DIR/featuremap-validate.log"
  check_overwrite "$report"

  resolve_featuremap_bin
  local rc=0
  run_wrapped "$report" "$log" "${FEATUREMAP_BIN[@]}" validate "$map" --scoring "$scoring" || rc=$?
  print_result "$( [[ $rc -eq 0 ]] && echo pass || echo fail )" "$report"
  exit "$rc"
}

# ── stage: map-check ─────────────────────────────────────────────────────

cmd_map_check() {
  local map="$BUNDLE/feature-map.yaml"
  local rom="" script="" expect=""
  local args=("${STAGE_ARGS[@]}")
  local i=0
  while [[ $i -lt ${#args[@]} ]]; do
    case "${args[$i]}" in
      --rom) rom="${args[$((i+1))]}"; i=$((i+2)) ;;
      --script) script="${args[$((i+1))]}"; i=$((i+2)) ;;
      --expect) expect="${args[$((i+1))]}"; i=$((i+2)) ;;
      --map) map="${args[$((i+1))]}"; i=$((i+2)) ;;
      --help)
        cat <<EOF
Usage: m6-session-pipeline.sh map-check --private-root <dir> \\
  --rom <private-rom> --script <private-script> --expect <expect.yaml> [--map <path>]

Runs the real \`refwork-verify map-check\` against the approved ROM and
operator script (fast-follow 03 step 4). --rom/--script/--expect are
private paths and must be supplied explicitly; --map defaults to
\$BUNDLE/feature-map.yaml.

NOTE: map-check has no native --report flag (refwork-verify/src/main.rs
cmd_map_check, no report_path variable). This stage wraps stdout/exit code
into a private report at \$BUNDLE/validation/map-check.json instead — see
fast-follow 03 step 4's own note that a report mode should be added before
claiming this gate on a bare textual PASS/FAIL.
EOF
        exit 0
        ;;
      *) die "unknown flag '${args[$i]}'" ;;
    esac
  done

  [[ -n "$rom" ]] || die "--rom is required (private ROM path)"
  [[ -n "$script" ]] || die "--script is required (private operator padlog script)"
  [[ -n "$expect" ]] || die "--expect is required (expectations.yaml; author beneath \$BUNDLE/validation/)"
  need_input "$rom" "ROM file" "operator-provided, private"
  need_input "$script" "map-check script" "operator-provided, private"
  need_input "$expect" "expectations file" "authored alongside feature-map.yaml per fast-follow 03 step 4"
  need_input "$map" "feature-map.yaml" "the map-authoring loop (SESSION-DAY-RUNBOOK.md)"

  local report="$VALIDATION_DIR/map-check.json"
  local log="$VALIDATION_DIR/map-check.log"
  check_overwrite "$report"

  resolve_verify_bin
  local rc=0
  run_wrapped "$report" "$log" "${VERIFY_BIN[@]}" map-check --rom "$rom" --map "$map" --script "$script" --expect "$expect" || rc=$?
  print_result "$( [[ $rc -eq 0 ]] && echo pass || echo fail )" "$report"
  exit "$rc"
}

# run_layout_review <layout.json> <real-map.yaml> <demo-map.yaml> <out-report.json>
#
# Mechanizes the fast-follow 03 step 6 independent review of a generated
# layout.json:
#   - every range addresses a registered region and is in bounds;
#   - range order equals feature-map order;
#   - total_len equals the sum of range lengths;
#   - map hash, capture-spec hash, exporter commit, and layout hash present;
#   - no offset in the real map reuses a placeholder offset from the
#     checked-in demo map, and the layout's compiled_from_feature_map_hash
#     is not the demo map's own hash.
# Exits 0 (review passed) or 1 (review found a problem), writing the full
# detail (which may include private offsets) only to <out-report.json>.
run_layout_review() {
  local layout="$1" real_map="$2" demo_map="$3" out_report="$4"
  python3 - "$layout" "$real_map" "$demo_map" "$out_report" <<'PYEOF'
import json
import subprocess
import sys

import yaml

layout_path, real_map_path, demo_map_path, out_path = sys.argv[1:5]

issues = []


def b3(path):
    out = subprocess.run(
        ["b3sum", "--no-names", path], capture_output=True, text=True, check=True
    )
    return out.stdout.strip()


with open(layout_path, encoding="utf-8") as f:
    layout = json.load(f)
with open(real_map_path, encoding="utf-8") as f:
    real_map = yaml.safe_load(f)
with open(demo_map_path, encoding="utf-8") as f:
    demo_map = yaml.safe_load(f)

# Required fields present.
for field in (
    "compiled_from_feature_map_hash",
    "capture_spec_hash",
    "compiler_or_exporter_commit",
    "blake3",
    "total_len",
    "ranges",
):
    if not layout.get(field) and layout.get(field) != 0:
        issues.append(f"layout.json missing required field: {field}")

ranges = layout.get("ranges", [])
features = real_map.get("features", [])

if len(ranges) != len(features):
    issues.append(
        f"range count ({len(ranges)}) does not match feature count ({len(features)})"
    )

# total_len equals sum of range lengths.
computed_total = sum(int(r.get("len", 0)) for r in ranges)
if computed_total != layout.get("total_len"):
    issues.append(
        f"total_len mismatch: layout.json says {layout.get('total_len')}, "
        f"sum of range lens is {computed_total}"
    )

# Region sizes for in-bounds check.
region_sizes = {}
for region in real_map.get("regions", []):
    region_sizes[region["name"]] = int(region["size"])

# Order + in-bounds: pair ranges with features by index.
for idx, (feat, rng) in enumerate(zip(features, ranges)):
    feat_region = feat.get("region")
    feat_offset = int(feat.get("offset"))
    rng_region = rng.get("region")
    rng_offset = int(rng.get("offset"))
    rng_len = int(rng.get("len", 0))

    if feat_region != rng_region:
        issues.append(
            f"range order mismatch at index {idx}: feature region "
            f"'{feat_region}' vs range region '{rng_region}'"
        )
    if feat_offset != rng_offset:
        issues.append(
            f"range order mismatch at index {idx}: feature offset "
            f"vs range offset differ (feature '{feat.get('name')}')"
        )

    size = region_sizes.get(rng_region)
    if size is None:
        issues.append(f"range at index {idx} references undeclared region '{rng_region}'")
    elif rng_offset + rng_len > size:
        issues.append(
            f"range at index {idx} (region '{rng_region}') is out of bounds: "
            f"offset+len={rng_offset + rng_len} > region size={size}"
        )

# No reused placeholder offsets from the checked-in demo map.
demo_offsets = {int(f.get("offset")) for f in demo_map.get("features", []) if "offset" in f}
real_offsets = {int(f.get("offset")) for f in features if "offset" in f}
collisions = sorted(real_offsets & demo_offsets)
if collisions:
    issues.append(
        "real map reuses placeholder offset(s) from feature-maps/demo-game.yaml: "
        + ", ".join(hex(c) for c in collisions)
    )

# Layout must not embed the demo map's own hash.
demo_hash = "blake3:" + b3(demo_map_path)
if layout.get("compiled_from_feature_map_hash") == demo_hash:
    issues.append(
        "layout.json's compiled_from_feature_map_hash equals the checked-in "
        "demo map's hash — the real map was not actually compiled"
    )

report = {
    "stage": "layout-review",
    "status": "pass" if not issues else "fail",
    "range_count": len(ranges),
    "feature_count": len(features),
    "total_len_checked": computed_total,
    "issues": issues,
}
with open(out_path, "w", encoding="utf-8") as f:
    json.dump(report, f, indent=2)
    f.write("\n")

if issues:
    print(f"layout-review: FAIL — {len(issues)} issue(s); see {out_path}")
    sys.exit(1)
else:
    print(f"layout-review: PASS — ranges={len(ranges)}")
    sys.exit(0)
PYEOF
}

# ── stage: layout ────────────────────────────────────────────────────────

cmd_layout() {
  local map="$BUNDLE/feature-map.yaml"
  local out="$BUNDLE/layout.json"
  local capture_spec_hash=""
  local layout_version="1"
  local exporter_commit="$DEFAULT_EXPORTER_COMMIT"
  local args=("${STAGE_ARGS[@]}")
  local i=0
  while [[ $i -lt ${#args[@]} ]]; do
    case "${args[$i]}" in
      --map) map="${args[$((i+1))]}"; i=$((i+2)) ;;
      --out) out="${args[$((i+1))]}"; i=$((i+2)) ;;
      --capture-spec-hash) capture_spec_hash="${args[$((i+1))]}"; i=$((i+2)) ;;
      --layout-version) layout_version="${args[$((i+1))]}"; i=$((i+2)) ;;
      --exporter-commit) exporter_commit="${args[$((i+1))]}"; i=$((i+2)) ;;
      --help)
        cat <<EOF
Usage: m6-session-pipeline.sh layout --private-root <dir> \\
  --capture-spec-hash <hash-or-ref> [--map <path>] [--out <path>] \\
  [--layout-version N] [--exporter-commit <sha>]

Generates \$BUNDLE/layout.json via \`refwork-verify phase4-layout\` citing
the exporter commit (default: $DEFAULT_EXPORTER_COMMIT, the refwork-czi
commit — fast-follow 03 step 5), then runs the independent-review checks
from fast-follow 03 step 6 mechanically:
  - every range is in bounds for its region's size;
  - range order equals feature-map order;
  - total_len equals the sum of range lengths;
  - map hash / capture-spec hash / exporter commit / layout hash are present;
  - no offset in the real map reuses a placeholder offset from the
    checked-in feature-maps/demo-game.yaml, and the layout's
    compiled_from_feature_map_hash does not equal the demo map's hash.
EOF
        exit 0
        ;;
      *) die "unknown flag '${args[$i]}'" ;;
    esac
  done

  [[ -n "$capture_spec_hash" ]] || die "--capture-spec-hash is required (opaque CaptureSpec contract ref/hash; not defaulted)"
  need_input "$map" "feature-map.yaml" "the map-authoring loop (SESSION-DAY-RUNBOOK.md)"
  need_input "$DEMO_MAP" "checked-in demo feature map" "repository checkout (feature-maps/demo-game.yaml) — should always exist"

  local gen_report="$VALIDATION_DIR/layout-generate.json"
  local gen_log="$VALIDATION_DIR/layout-generate.log"
  local review_report="$VALIDATION_DIR/layout-review.json"
  check_overwrite "$out"
  check_overwrite "$gen_report"
  check_overwrite "$review_report"

  resolve_verify_bin
  local rc=0
  run_wrapped "$gen_report" "$gen_log" "${VERIFY_BIN[@]}" phase4-layout \
    --map "$map" --out "$out" --capture-spec-hash "$capture_spec_hash" \
    --layout-version "$layout_version" --compiler-or-exporter-commit "$exporter_commit" || rc=$?

  if [[ "$rc" -ne 0 ]]; then
    print_result fail "$gen_report"
    exit "$rc"
  fi

  # Independent review (fast-follow 03 step 6), mechanized inline (kept in
  # this single script rather than a companion file).
  local review_rc=0
  run_layout_review "$out" "$map" "$DEMO_MAP" "$review_report" \
    > "$VALIDATION_DIR/layout-review.log" 2>&1 || review_rc=$?

  if [[ "$review_rc" -eq 0 ]]; then
    print_result pass "$review_report"
  else
    print_result fail "$review_report"
  fi
  exit "$review_rc"
}

# ── stage: capture ───────────────────────────────────────────────────────

cmd_capture() {
  local endpoint="$DEFAULT_ENDPOINT"
  local snapshot=""
  local padlog="" map="$BUNDLE/feature-map.yaml" layout="$BUNDLE/layout.json"
  local bundle_out="$BUNDLE"
  local count="1000" cadence="1" hard_icount_cap="" source_ref=""
  local production=0
  local args=("${STAGE_ARGS[@]}")
  local i=0
  while [[ $i -lt ${#args[@]} ]]; do
    case "${args[$i]}" in
      --endpoint) endpoint="${args[$((i+1))]}"; i=$((i+2)) ;;
      --snapshot) snapshot="${args[$((i+1))]}"; i=$((i+2)) ;;
      --padlog) padlog="${args[$((i+1))]}"; i=$((i+2)) ;;
      --map) map="${args[$((i+1))]}"; i=$((i+2)) ;;
      --layout) layout="${args[$((i+1))]}"; i=$((i+2)) ;;
      --bundle) bundle_out="${args[$((i+1))]}"; i=$((i+2)) ;;
      --count) count="${args[$((i+1))]}"; i=$((i+2)) ;;
      --cadence) cadence="${args[$((i+1))]}"; i=$((i+2)) ;;
      --hard-icount-cap) hard_icount_cap="${args[$((i+1))]}"; i=$((i+2)) ;;
      --source-ref) source_ref="${args[$((i+1))]}"; i=$((i+2)) ;;
      --production) production=1; i=$((i+1)) ;;
      --help)
        cat <<EOF
Usage: m6-session-pipeline.sh capture --private-root <dir> \\
  --padlog <private-padlog> --hard-icount-cap <N> --source-ref <opaque> \\
  [--endpoint <worker>] [--snapshot <blake3-or-ref>] [--map <path>] \\
  [--layout <path>] [--bundle <dir>] [--count N] [--cadence N] [--production]

Wraps \`refwork-verify phase4-capture-export\` (fast-follow 04 step 3).
Framebuffer capture is always on for every primary row (hardcoded in
refwork-verify/src/phase4_capture_export.rs — CaptureExportOptions has no
framebuffer flag), so there is nothing to toggle here.

Defaults:
  --endpoint   $DEFAULT_ENDPOINT
  --snapshot   read BRIDGE_REAL_SNAPSHOT_REF from
               $DEFAULT_REAL_ENV (override with REAL_ENV_PATH)
  --map        \$BUNDLE/feature-map.yaml
  --layout     \$BUNDLE/layout.json
  --bundle     \$BUNDLE (captures/, artifacts/framebuffer/, validation/
               capture-export-report.json all land here)
  --count      1000 (fast-follow 04 step 3 minimum; refuses below 1000)
  --cadence    1 (override — choose cadence to cover transitions, not
               near-identical frames, per fast-follow 04 step 3)
EOF
        exit 0
        ;;
      *) die "unknown flag '${args[$i]}'" ;;
    esac
  done

  if [[ -z "$snapshot" ]]; then
    local real_env="${REAL_ENV_PATH:-$DEFAULT_REAL_ENV}"
    need_input "$real_env" "READY snapshot handoff env file" "M4 regen handoff (BRIDGE_REAL_SNAPSHOT_REF)"
    snapshot="$(grep -m1 '^BRIDGE_REAL_SNAPSHOT_REF=' "$real_env" | cut -d= -f2-)"
    [[ -n "$snapshot" ]] || die "BRIDGE_REAL_SNAPSHOT_REF not set in $real_env; pass --snapshot explicitly"
  fi

  [[ -n "$padlog" ]] || die "--padlog is required (private recorded padlog for the coherent session)"
  [[ -n "$hard_icount_cap" ]] || die "--hard-icount-cap is required (safety cap; see fast-follow 01 step 3 worker-provenance note — do not silently default a safety bound)"
  [[ -n "$source_ref" ]] || die "--source-ref is required (opaque provenance ref for this capture run)"
  if [[ "$count" -lt 1000 ]]; then
    die "--count must be >= 1000 (fast-follow 04 step 3 minimum primary real captures); got $count"
  fi

  need_input "$padlog" "recorded padlog" "the operator hand-play session"
  need_input "$map" "feature-map.yaml" "the map-authoring loop (SESSION-DAY-RUNBOOK.md)"
  need_input "$layout" "layout.json" "the 'layout' pipeline stage"

  local native_report="$bundle_out/validation/capture-export-report.json"
  mkdir -p "$bundle_out/validation"
  check_overwrite "$native_report"

  resolve_verify_bin
  local cmd=("${VERIFY_BIN[@]}" phase4-capture-export
    --endpoint "$endpoint" --snapshot "$snapshot" --padlog "$padlog"
    --map "$map" --layout "$layout" --bundle "$bundle_out"
    --count "$count" --cadence "$cadence" --hard-icount-cap "$hard_icount_cap"
    --source-ref "$source_ref")
  [[ "$production" -eq 1 ]] && cmd+=(--production)

  local log="$VALIDATION_DIR/capture-export.log"
  local rc=0
  "${cmd[@]}" >"$log" 2>&1 || rc=$?

  if [[ "$rc" -eq 0 ]]; then
    print_result pass "$native_report"
  else
    print_result fail "$native_report"
  fi
  exit "$rc"
}

# ── stage: artifact-check ────────────────────────────────────────────────

cmd_artifact_check() {
  local bundle_in="$BUNDLE"
  local report="$VALIDATION_DIR/artifact-check.json"
  local args=("${STAGE_ARGS[@]}")
  local i=0
  while [[ $i -lt ${#args[@]} ]]; do
    case "${args[$i]}" in
      --bundle) bundle_in="${args[$((i+1))]}"; i=$((i+2)) ;;
      --report) report="${args[$((i+1))]}"; i=$((i+2)) ;;
      --help)
        cat <<EOF
Usage: m6-session-pipeline.sh artifact-check --private-root <dir> [--bundle <dir>] [--report <path>]

Runs \`refwork-verify phase4-artifact-check\` (fast-follow 04 step 4).
Defaults: --bundle \$BUNDLE, --report \$BUNDLE/validation/artifact-check.json.
EOF
        exit 0
        ;;
      *) die "unknown flag '${args[$i]}'" ;;
    esac
  done

  need_input "$bundle_in/captures" "captures directory" "the 'capture' pipeline stage"
  check_overwrite "$report"

  resolve_verify_bin
  local rc=0
  "${VERIFY_BIN[@]}" phase4-artifact-check --bundle "$bundle_in" --report "$report" \
    > "$VALIDATION_DIR/artifact-check.log" 2>&1 || rc=$?

  if [[ "$rc" -eq 0 ]]; then
    print_result pass "$report"
  else
    print_result fail "$report"
  fi
  exit "$rc"
}

# ── stage: score-plan ────────────────────────────────────────────────────

cmd_score_plan() {
  local captures="$BUNDLE/captures/index.jsonl"
  local out="$BUNDLE/score-plan.json"
  local client_batch_prefix="phase4-k32"
  local first_boss=() goal_positive=() goal_negative=()
  local checkpoint_after_batch=""
  local restore_control_batch=()
  local args=("${STAGE_ARGS[@]}")
  local i=0
  while [[ $i -lt ${#args[@]} ]]; do
    case "${args[$i]}" in
      --captures) captures="${args[$((i+1))]}"; i=$((i+2)) ;;
      --out) out="${args[$((i+1))]}"; i=$((i+2)) ;;
      --client-batch-prefix) client_batch_prefix="${args[$((i+1))]}"; i=$((i+2)) ;;
      --first-boss) first_boss+=("${args[$((i+1))]}"); i=$((i+2)) ;;
      --goal-positive) goal_positive+=("${args[$((i+1))]}"); i=$((i+2)) ;;
      --goal-negative) goal_negative+=("${args[$((i+1))]}"); i=$((i+2)) ;;
      --checkpoint-after-batch) checkpoint_after_batch="${args[$((i+1))]}"; i=$((i+2)) ;;
      --restore-control-batch) restore_control_batch+=("${args[$((i+1))]}"); i=$((i+2)) ;;
      --help)
        cat <<EOF
Usage: m6-session-pipeline.sh score-plan --private-root <dir> \\
  --first-boss <capture-id> --goal-positive <capture-id> --goal-negative <capture-id> \\
  [--captures <path>] [--out <path>] [--client-batch-prefix <prefix>] \\
  [--checkpoint-after-batch <id>] [--restore-control-batch <id> ...]

Runs the K=32 \`refwork-verify phase4-score-plan\` (fast-follow 04 step 7).
--first-boss/--goal-positive/--goal-negative may repeat (matches the
underlying CLI's Vec<String> options; refwork-verify/src/main.rs
cmd_phase4_score_plan). Defaults: --captures \$BUNDLE/captures/index.jsonl,
--out \$BUNDLE/score-plan.json.

NOTE: phase4-score-plan has no native --report flag; this stage wraps
stdout/exit code into \$BUNDLE/validation/score-plan-report.json.
EOF
        exit 0
        ;;
      *) die "unknown flag '${args[$i]}'" ;;
    esac
  done

  [[ ${#first_boss[@]} -gt 0 ]] || die "--first-boss <capture-id> is required (at least one)"
  [[ ${#goal_positive[@]} -gt 0 ]] || die "--goal-positive <capture-id> is required (at least one)"
  [[ ${#goal_negative[@]} -gt 0 ]] || die "--goal-negative <capture-id> is required (at least one)"
  need_input "$captures" "capture index" "the 'capture' pipeline stage"

  local report="$VALIDATION_DIR/score-plan-report.json"
  local log="$VALIDATION_DIR/score-plan.log"
  check_overwrite "$out"
  check_overwrite "$report"

  resolve_verify_bin
  local cmd=("${VERIFY_BIN[@]}" phase4-score-plan --captures "$captures" --out "$out" --client-batch-prefix "$client_batch_prefix")
  for id in "${first_boss[@]}"; do cmd+=(--first-boss "$id"); done
  for id in "${goal_positive[@]}"; do cmd+=(--goal-positive "$id"); done
  for id in "${goal_negative[@]}"; do cmd+=(--goal-negative "$id"); done
  [[ -n "$checkpoint_after_batch" ]] && cmd+=(--checkpoint-after-batch "$checkpoint_after_batch")
  for id in "${restore_control_batch[@]}"; do cmd+=(--restore-control-batch "$id"); done

  local rc=0
  run_wrapped "$report" "$log" "${cmd[@]}" || rc=$?
  print_result "$( [[ $rc -eq 0 ]] && echo pass || echo fail )" "$report"
  exit "$rc"
}

# ── stage: trace ─────────────────────────────────────────────────────────

cmd_trace() {
  local captures="$BUNDLE/captures/index.jsonl"
  local map="$BUNDLE/feature-map.yaml"
  local scoring="$BUNDLE/scoring-program.yaml"
  local labels=""
  local out="$BUNDLE/trajectory/first-boss.jsonl"
  local report="$VALIDATION_DIR/trace-report.json"
  local args=("${STAGE_ARGS[@]}")
  local i=0
  while [[ $i -lt ${#args[@]} ]]; do
    case "${args[$i]}" in
      --captures) captures="${args[$((i+1))]}"; i=$((i+2)) ;;
      --map) map="${args[$((i+1))]}"; i=$((i+2)) ;;
      --scoring) scoring="${args[$((i+1))]}"; i=$((i+2)) ;;
      --labels) labels="${args[$((i+1))]}"; i=$((i+2)) ;;
      --out) out="${args[$((i+1))]}"; i=$((i+2)) ;;
      --report) report="${args[$((i+1))]}"; i=$((i+2)) ;;
      --help)
        cat <<EOF
Usage: m6-session-pipeline.sh trace --private-root <dir> --labels <private-labels.yaml> \\
  [--captures <path>] [--map <path>] [--scoring <path>] [--out <path>] [--report <path>]

Runs \`refwork-verify trace\` (fast-follow 04 step 8). Defaults:
--captures \$BUNDLE/captures/index.jsonl, --map \$BUNDLE/feature-map.yaml,
--scoring \$BUNDLE/scoring-program.yaml,
--out \$BUNDLE/trajectory/first-boss.jsonl,
--report \$BUNDLE/validation/trace-report.json.
EOF
        exit 0
        ;;
      *) die "unknown flag '${args[$i]}'" ;;
    esac
  done

  [[ -n "$labels" ]] || die "--labels is required (private operator labels joined by capture id; fast-follow 04 step 6)"
  need_input "$captures" "capture index" "the 'capture' pipeline stage"
  need_input "$map" "feature-map.yaml" "the map-authoring loop (SESSION-DAY-RUNBOOK.md)"
  need_input "$scoring" "scoring-program.yaml" "the map-authoring loop (SESSION-DAY-RUNBOOK.md)"
  need_input "$labels" "operator labels" "fast-follow 04 step 6 (private operator labeling)"

  check_overwrite "$out"
  check_overwrite "$report"
  mkdir -p "$(dirname "$out")"

  resolve_verify_bin
  local rc=0
  "${VERIFY_BIN[@]}" trace --captures "$captures" --map "$map" --scoring "$scoring" \
    --labels "$labels" --out "$out" --report "$report" \
    > "$VALIDATION_DIR/trace.log" 2>&1 || rc=$?

  if [[ "$rc" -eq 0 ]]; then
    print_result pass "$report"
  else
    print_result fail "$report"
  fi
  exit "$rc"
}

# ── stage: status ────────────────────────────────────────────────────────

cmd_status() {
  for a in "${STAGE_ARGS[@]}"; do
    if [[ "$a" == "--help" ]]; then
      cat <<EOF
Usage: m6-session-pipeline.sh status --private-root <dir>

Shows PASS/FAIL/MISSING for each pipeline stage's report/output under
\$BUNDLE, so the pipeline is resumable and progress is visible without
re-running anything.
EOF
      exit 0
    fi
  done

  printf '%-16s %-10s %s\n' "STAGE" "STATUS" "REPORT/OUTPUT"

  local s

  s="$(report_status "$VALIDATION_DIR/featuremap-validate.json")"
  printf '%-16s %-10s %s\n' "validate-map" "$s" "$VALIDATION_DIR/featuremap-validate.json"

  s="$(report_status "$VALIDATION_DIR/map-check.json")"
  printf '%-16s %-10s %s\n' "map-check" "$s" "$VALIDATION_DIR/map-check.json"

  local layout_gen layout_review layout_s
  layout_gen="$(report_status "$VALIDATION_DIR/layout-generate.json")"
  layout_review="$(report_status "$VALIDATION_DIR/layout-review.json")"
  if [[ "$layout_gen" == "PASS" && "$layout_review" == "PASS" ]]; then
    layout_s="PASS"
  elif [[ "$layout_gen" == "MISSING" && "$layout_review" == "MISSING" ]]; then
    layout_s="MISSING"
  else
    layout_s="FAIL"
  fi
  printf '%-16s %-10s %s\n' "layout" "$layout_s" "$BUNDLE/layout.json (gen=$layout_gen review=$layout_review)"

  s="$(report_status "$BUNDLE/validation/capture-export-report.json")"
  printf '%-16s %-10s %s\n' "capture" "$s" "$BUNDLE/validation/capture-export-report.json"

  s="$(report_status "$VALIDATION_DIR/artifact-check.json")"
  printf '%-16s %-10s %s\n' "artifact-check" "$s" "$VALIDATION_DIR/artifact-check.json"

  local score_plan_s="MISSING"
  if [[ -s "$BUNDLE/score-plan.json" ]]; then
    score_plan_s="$(report_status "$VALIDATION_DIR/score-plan-report.json")"
    [[ "$score_plan_s" == "MISSING" ]] && score_plan_s="UNREADABLE"
  fi
  printf '%-16s %-10s %s\n' "score-plan" "$score_plan_s" "$BUNDLE/score-plan.json"

  local trace_s
  trace_s="$(report_status "$VALIDATION_DIR/trace-report.json")"
  if [[ "$trace_s" == "PASS" && ! -s "$BUNDLE/trajectory/first-boss.jsonl" ]]; then
    trace_s="FAIL"
  fi
  printf '%-16s %-10s %s\n' "trace" "$trace_s" "$BUNDLE/trajectory/first-boss.jsonl"

  exit 0
}

# ── dispatch ─────────────────────────────────────────────────────────────

case "$STAGE" in
  validate-map) cmd_validate_map ;;
  map-check) cmd_map_check ;;
  layout) cmd_layout ;;
  capture) cmd_capture ;;
  artifact-check) cmd_artifact_check ;;
  score-plan) cmd_score_plan ;;
  trace) cmd_trace ;;
  status) cmd_status ;;
esac
