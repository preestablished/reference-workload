#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: run-authoritative.sh --rom PATH --case NAME --warmup N \
  --lengths N1,N2,N3 [--script PATH] [--repetitions N] [--out DIR]

Builds the release benchmark once, then records raw perf stat and benchmark JSON
for each exact window. The default input is the deterministic synthetic schedule.
EOF
}

rom=
case_name=
warmup=
lengths=
script=
repetitions=3
out=

while (($#)); do
  case "$1" in
    --rom) rom=${2:?}; shift 2 ;;
    --case) case_name=${2:?}; shift 2 ;;
    --warmup) warmup=${2:?}; shift 2 ;;
    --lengths) lengths=${2:?}; shift 2 ;;
    --script) script=${2:?}; shift 2 ;;
    --repetitions) repetitions=${2:?}; shift 2 ;;
    --out) out=${2:?}; shift 2 ;;
    --help|-h) usage; exit 0 ;;
    *) echo "unknown option: $1" >&2; usage >&2; exit 2 ;;
  esac
done

[[ -n $rom && -n $case_name && -n $warmup && -n $lengths ]] || {
  usage >&2
  exit 2
}
[[ -f $rom ]] || { echo "ROM not found: $rom" >&2; exit 2; }
[[ $repetitions =~ ^[1-9][0-9]*$ ]] || { echo "invalid repetitions" >&2; exit 2; }
if [[ -n $script && ! -f $script ]]; then
  echo "script not found" >&2
  exit 2
fi

root=$(git rev-parse --show-toplevel)
cd "$root"
run_id=$(date -u +%Y%m%dT%H%M%SZ)-"$case_name"
out=${out:-target/refwork-emu-perf/$run_id}
mkdir -p "$out/raw"

cargo build --locked --release -p refwork-emu-bench
binary=$root/target/release/refwork-emu-bench
binary_sha256=$(sha256sum "$binary" | awk '{print $1}')
rom_blake3=$(b3sum "$rom" | awk '{print $1}')
script_blake3=null
input_args=(--synthetic-input)
if [[ -n $script ]]; then
  script_blake3=$(b3sum "$script" | awk '{print $1}')
  input_args=(--script "$script")
fi

{
  echo "schema=1"
  echo "git_rev=$(git rev-parse HEAD)"
  echo "rustc=$(rustc -Vv | tr '\n' ';')"
  echo "cargo=$(cargo -V)"
  echo "kernel=$(uname -srvmo)"
  echo "perf=$(perf --version)"
  echo "cpu_model=$(sed -n 's/^model name[[:space:]]*: //p' /proc/cpuinfo | head -1)"
  echo "binary=$binary"
  echo "binary_sha256=$binary_sha256"
  echo "rom_blake3=$rom_blake3"
  echo "script_blake3=$script_blake3"
  echo "case=$case_name"
  echo "warmup_frames=$warmup"
  echo "lengths=$lengths"
  echo "repetitions=$repetitions"
} > "$out/manifest.txt"

IFS=, read -r -a window_lengths <<< "$lengths"
default_cpu=$(awk '/Cpus_allowed_list/ { split($2, ranges, ","); split(ranges[1], first, "-"); print first[1] }' /proc/self/status)
perf_cpu=${REFWORK_PERF_CPU:-$default_cpu}
for length in "${window_lengths[@]}"; do
  [[ $length =~ ^[1-9][0-9]*$ ]] || { echo "invalid length: $length" >&2; exit 2; }
  for ((run = 1; run <= repetitions; run++)); do
    stem=$out/raw/window-${length}-run-${run}
    control=$stem.control.fifo
    ack=$stem.ack.fifo
    rm -f "$control" "$ack"
    mkfifo "$control" "$ack"
    taskset -c "$perf_cpu" \
      perf stat -D -1 --control "fifo:$control,$ack" \
      --no-big-num -x, -e instructions:u --output "$stem.perf.csv" -- \
      "$binary" --rom "$rom" --case "$case_name" \
      --warmup-frames "$warmup" --measure-frames "$length" \
      --perf-control "$control" --perf-ack "$ack" \
      "${input_args[@]}" > "$stem.json" 2> "$stem.stderr"
    rm -f "$control" "$ack"
    grep -q 'instructions:u' "$stem.perf.csv" || {
      echo "missing instructions:u in $stem.perf.csv" >&2
      exit 1
    }
    if grep -Eq '<not supported>|<not counted>' "$stem.perf.csv" ||
      ! awk -F, '$3 == "instructions:u" { found=1; if ($5 != "100.00") exit 1 } END { if (!found) exit 1 }' "$stem.perf.csv"; then
      echo "unsupported, uncounted, or scaled perf result: $stem.perf.csv" >&2
      exit 1
    fi
    taskset -c "$perf_cpu" "$binary" --rom "$rom" --case "$case_name" \
      --warmup-frames "$warmup" --measure-frames "$length" \
      "${input_args[@]}" > "$stem.wall.json" 2> "$stem.wall.stderr"
  done
  reference=$out/raw/window-${length}-run-1.json
  for ((run = 2; run <= repetitions; run++)); do
    cmp -s "$reference" "$out/raw/window-${length}-run-${run}.json" || {
      echo "deterministic result mismatch at window $length run $run" >&2
      exit 1
    }
  done
done

echo "$out"
