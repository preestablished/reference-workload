# Verification And Handoff Shape

## Phases-Track Verification

On your resolution we will:

1. Re-run `refwork-verify trace` from a clean checkout against the
   recorded hand-play session artifacts and diff the emitted JSONL
   against your frozen fixture (hash match).
2. Re-evaluate the frozen trajectory through the scorer build you name
   (SHA + loaded map/program hashes) and confirm monotonic stage
   progression and goal-fires-only-on-credits from scratch.
3. Cross-check the fixture-corpus id/hashes against the fast-follow's
   FULFILLMENT records — one consistent corpus id everywhere.
4. Read the smoke evidence: burst count, `Fault` count, spot-replay
   hash-mismatch count, and the stack/window coordination note.
5. Confirm Phase 4 exit gate 3 can be declared from the recorded
   evidence verbatim, and mark it in the phase tracking.

## Choreography With Siblings

- **Predecessor within this repo:**
  `phase4-real-capture-corpus-fast-follow/` — produces every private
  input this request consumes (exporter, real map, hand-play session,
  frozen corpus). One operator session should feed both; brief the
  operator on M6's trajectory/labeling needs (through first boss +
  credits fixture) before that session runs — that is why this packet is
  filed before its gate opens.
- **Joint partner:** `state-scorer/.agents/requests/phase4-m1-m4-first-boss-scoring/`
  — items 1–2 and 4 are two-sided; the scorer packet's item 5 (joint
  smoke with the orchestrator) and this request's item 5 can share a
  window. Spec-ownership rule recorded there too: this repo's API.md
  governs DSL semantics.
- **input-synthesizer `phase4-m0-m2-v1-generators/`** — optional
  better-evidence input source for item 5; never a gate.
- **exploration-orchestrator** — dev-loop counterpart for item 5;
  coordinate around the open leak-verification bead
  (`rom-operator-bridge-l1w`) and shared-box scheduling per the 07-10
  program flags.
- **control-plane** — item 6's registration target if their resource API
  ships in time.

## Handback Shape

Append `04-resolution.md` here: git SHAs (this repo + the scorer build
used), bead states, which entry-gate branch held (full corpus vs
first-room fallback), frozen-artifact ids/hashes (labeled trajectory,
fixture corpus), the four validation results, smoke counts, and the
handoff-surface disposition (registered vs manifest+dist). We respond
with `05-verification.md` after the checks above. Phase 4's exit is
declared only after gates 1–4 are all verified — this request closes
gate 3 and contributes evidence to gates 1–2; say plainly in the
resolution which gate items you believe are now green.
