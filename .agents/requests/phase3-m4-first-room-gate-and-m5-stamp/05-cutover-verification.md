# Cutover Verification — Gate 3 Human-Visible Half CLOSED (2026-07-12)

The remaining ask from `04-progress-and-operator-ask.md` ("Remaining Ask,
final shrink") is fully resolved:

1. **Bridge cutover window — DONE 2026-07-12 ~02:22Z.** The operator cut
   `BRIDGE_REAL_SNAPSHOT_REF` over to
   `948b73e6238a6c8bb1fe67f0c104e0721b8c2bfd745870dba131eac417bdfbfa` (the
   07-07 regen READY snapshot; the same ref the passing `vm-first-room`
   validating run restored) and confirmed **live frames rendering from the
   real ROM in the deployed browser**. Worker `ListSlots` showed the session
   slot RUNNING with that snapshot as base and advancing icount.
   Per the verification offer, the pre-cutover ref was recorded rather than
   assumed: `868d7370…` (a 07-06 regen from the pre-`7b0c7b2` image — it was
   never gate-validated, so the cutover was to a different, newer ref).
   Full record incl. cutover incidents and the durable snapstore location:
   `rom-operator-bridge/.agents/handoffs/2026-07-12-real-snapshot-cutover-confirmation.md`.
   Bridge beads `9xo` and `bvq` closed against it.
2. **M2 build-vs-vendor / aarch64 decisions** — resolved by the operator;
   `refwork-d7t.1` closed (2026-07-11/12), which closed the `refwork-d7t` epic.

With this, all four Phase 3 exit gates are green: M5 20× zero-flake stamp
(gate 1), guest-sdk Ms4/Ms5 acceptance (gate 2), first room in-VM through the
worker gRPC API **and** visible in the deployed browser (gate 3), and
snapshot-store M7 GC property tests (gate 4). Phase 3 is complete.
