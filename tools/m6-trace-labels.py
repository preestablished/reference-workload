#!/usr/bin/env python3
"""m6-trace-labels.py — build a `phase4-trace-labels` YAML for `refwork-verify trace`
from an event table and a host capture index.

Context: ~/.agents/projects/reference-workload/plans/discovery-02-processing-and-interim-scoring/
package 06 (WP5). Labels are derived from operator/watch-log EVENT FRAMES, never
from the scoring program, so `refwork-verify trace` compares two independent
derivations (label vs program-computed) per capture.

Inputs (both private; paths are given on the command line, never echoed):
  --events <events.yaml>   kind: m6-trace-events, schema_version: 1
      session: <name>
      stage_order: [stage names in scoring-program order]
      stage_frames: {stage: <first frame at which its latch holds> | null}
      goal_frame: <first frame at which the goal holds> | null
      prune_windows: [[start, end|null], ...]   # inclusive frame windows where prune holds
  --index <index.jsonl>    rows with capture_id + frame_index (host-capture-index output)
Output:
  --out <labels.yaml>      kind: phase4-trace-labels, schema_version: 1, one label per capture

Rules per capture frame f (frame_index; same 0-based frame space as ramdiff marks):
  active_stages         = [s for s in stage_order if stage_frames[s] is not None and f >= stage_frames[s]]
  expected_highest_stage = active_stages[-1] or "root"
  goal                  = goal_frame is not None and f >= goal_frame
  prune                 = any(start <= f and (end is None or f <= end) for [start, end] in prune_windows)
  first_boss_coverage   = "first_boss" in active_stages   (phase4_trace.rs hardcodes this stage name)

STDOUT: row counts only (never frames, values, or private paths).
"""
import argparse
import json
import sys

import yaml


def load_events(path):
    ev = yaml.safe_load(open(path))
    if ev.get("kind") != "m6-trace-events" or ev.get("schema_version") != 1:
        sys.exit("events: kind/schema_version mismatch (want m6-trace-events v1)")
    order = list(ev["stage_order"])
    frames = dict(ev.get("stage_frames") or {})
    for s in order:
        if s not in frames:
            sys.exit(f"events: stage_frames missing stage {s!r}")
    for s in frames:
        if s not in order:
            sys.exit(f"events: stage_frames names unknown stage {s!r}")
    windows = []
    for w in ev.get("prune_windows") or []:
        if not isinstance(w, list) or len(w) != 2:
            sys.exit("events: prune_windows entries must be [start, end|null]")
        start, end = w
        if end is not None and end < start:
            sys.exit("events: prune window end < start")
        windows.append((int(start), None if end is None else int(end)))
    goal = ev.get("goal_frame")
    return order, frames, (None if goal is None else int(goal)), windows


def label_for(frame, order, frames, goal_frame, windows):
    active = [s for s in order if frames[s] is not None and frame >= int(frames[s])]
    return {
        "expected_highest_stage": active[-1] if active else "root",
        "active_stages": active,
        "prune": any(s <= frame and (e is None or frame <= e) for s, e in windows),
        "goal": goal_frame is not None and frame >= goal_frame,
        "first_boss_coverage": "first_boss" in active,
    }


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--events", required=True)
    ap.add_argument("--index", required=True)
    ap.add_argument("--out", required=True)
    args = ap.parse_args()

    order, frames, goal_frame, windows = load_events(args.events)
    labels = []
    seen = set()
    counts = {"prune": 0, "goal": 0, "first_boss_coverage": 0}
    with open(args.index) as fh:
        for line_no, line in enumerate(fh, 1):
            if not line.strip():
                continue
            row = json.loads(line)
            cid = row.get("capture_id")
            frame = row.get("frame_index", row.get("frame_counter"))
            if cid is None or frame is None:
                sys.exit(f"index:{line_no}: missing capture_id/frame_index")
            if cid in seen:
                sys.exit(f"index:{line_no}: duplicate capture_id")
            seen.add(cid)
            lab = label_for(int(frame), order, frames, goal_frame, windows)
            for k in counts:
                counts[k] += int(bool(lab[k]))
            labels.append({"capture_id": cid, **lab})
    doc = {"kind": "phase4-trace-labels", "schema_version": 1, "labels": labels}
    with open(args.out, "w") as fh:
        yaml.safe_dump(doc, fh, sort_keys=False)
    print(
        f"m6-trace-labels: {len(labels)} label(s) written; prune={counts['prune']} "
        f"goal={counts['goal']} first_boss_coverage={counts['first_boss_coverage']}"
    )


if __name__ == "__main__":
    main()
