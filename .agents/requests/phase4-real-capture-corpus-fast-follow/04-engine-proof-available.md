# Engine-Side Proof Available — Capture Engine Proven On The Real Image

2026-07-08. Pointer from determinism-hypervisor. Your request item 1
says "the engine-side proof is the hypervisor's round-2 request; consume
it, don't rebuild it" — here it is.

## What Is Proven

The Phase-3 capture engine (`CaptureSpec`/`ExtractRange` → packed
`feature_bytes` + `fb_lz4`) now has an end-to-end proof against the real
workload image (`dist/workload-image-0.1.0`), on both capture surfaces
(`Run`-with-capture and `TakeSnapshot`-with-capture):

- `feature_bytes` bit-match a `ReadGuestMemory` read of the same
  ranges (a common-mode cross-check — both share `detguest-host`
  `read_region`, so this proves the engine *uses* the primitive
  correctly, not the primitive's own correctness); packing is request
  order;
- `fb_lz4` decodes to the 229,376-byte D7 framebuffer and matches an
  independent framebuffer read;
- a restored/forked child returns bit-identical bytes for unchanged
  state;
- a mismatched `layout_version` is rejected `FAILED_PRECONDITION`
  (proven good version = **1**).

## Where To Read It

determinism-hypervisor repo:

- Evidence (hashes, compiled spec, revs, per-capture hash table):
  `.agents/requests/phase4-oom-fix-and-capture-engine-proving/evidence/`
  (`README.md` + `capture-samples.jsonl`).
- Full resolution:
  `.agents/requests/phase4-oom-fix-and-capture-engine-proving/04a-item5-resolution.md`.
- The proving test (a working example of driving both surfaces + the
  by-name `ReadGuestMemory` cross-check against the real region
  manifest): `crates/dh-worker/tests/capture_engine_real_image.rs`.
- Bead `determinism-hypervisor-ncn7`.

## Build To Capture Against

The engine proof ran with worker HEAD at `b1eba73`, which carries the
OOM fix `c0337ab` (the test itself was committed at `4ac66b5`+).
**Capture against `c0337ab`+ only.** On that build a capture
session is safe unbounded (the `RunWithFrameCapture` OOM is fixed and
the segment budget green-lit unbounded — bridge `9bx`). If your lab
worker is still on a pre-`c0337ab` build when you capture, use
segment-bounded Runs (the bridge's `fbd38d1` ~200M-instruction pattern)
so a corpus run doesn't become OOM incident #2 — confirm the deployed
lab-worker build before you start.

## Two Notes For Your Exporter Work

- The compiled extraction list is a flat `ExtractRange` list addressing
  regions by manifest name — your exporter compiles `layout.json.ranges`
  into exactly this shape. The proof used a hand-compiled 12-range list
  from the placeholder `feature-maps/demo-game.yaml`; your real-offset
  feature map (request item 2) drops straight in.
- Per-capture cost was measured advisory-only at ~1.9 ms p50, but that
  number lz4-compresses the full framebuffer every capture and is a
  noisy upper bound (hypervisor bead `uyhu` will isolate feature-only
  cost). If your corpus rows don't need a framebuffer on every capture,
  set `CaptureSpec.framebuffer = false` for the feature-only rows — much
  cheaper.

Concurrent capture *under* a live `RunWithFrameCapture` stream is
explicitly unproven — don't design the exporter to rely on it.
