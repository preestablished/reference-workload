#!/usr/bin/env bash
set -euo pipefail

if (($# < 5)); then
  echo "Usage: run-sampling.sh ROM CASE WARMUP MEASURE OUT [SCRIPT]" >&2
  exit 2
fi

rom=$1
case_name=$2
warmup=$3
measure=$4
out=$5
script=${6:-}
root=$(git rev-parse --show-toplevel)
cd "$root"
mkdir -p "$out"

cargo build --locked --release -p refwork-emu-bench
binary=$root/target/release/refwork-emu-bench
control=$out/perf-control.fifo
ack=$out/perf-ack.fifo
rm -f "$control" "$ack"
mkfifo "$control" "$ack"
trap 'rm -f "$control" "$ack"' EXIT

input_args=(--synthetic-input)
if [[ -n $script ]]; then
  input_args=(--script "$script")
fi

# Start disabled; the benchmark enables immediately before the measured frame
# loop, waits for perf's acknowledgement, and disables immediately after it.
perf record -D -1 -e instructions:u -g --call-graph dwarf \
  --control "fifo:$control,$ack" -o "$out/perf.data" -- \
  "$binary" --rom "$rom" --case "$case_name" \
  --warmup-frames "$warmup" --measure-frames "$measure" \
  --perf-control "$control" --perf-ack "$ack" "${input_args[@]}" \
  > "$out/result.json" 2> "$out/perf.stderr"
perf report --stdio --show-nr-samples -i "$out/perf.data" > "$out/perf-report.txt"
