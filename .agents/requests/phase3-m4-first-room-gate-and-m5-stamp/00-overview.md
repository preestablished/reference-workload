# Request: Close The M4 First-Room Gate And Stamp M5 — You Are The Long Pole

## Who Is Asking

The `rom-operator-bridge` project (Phase 3 validation surface) on behalf of
the phases track. Filed 2026-07-07. This continues the plan already in your
repo at `.agents/plans/phase3-m4-first-room-unblock/` — nothing here
replaces that plan; this request updates its context (everything upstream
has since gone green), sharpens acceptance, and states what the bridge runs
when you're done.

## Why reference-workload, Why Now

Your own plan's closeout doc already says it
(`.agents/plans/phase3-m4-first-room-unblock/06-verification-and-closeout.md`):
**"reference-workload is the long pole now."** Every upstream dependency
has landed since:

- guest-sdk Ms4 — done, independently verified 2026-07-02
  (`../guest-sdk/.agents/requests/phase3-ms4-region-publication-acceptance/06-verification.md`).
- guest-sdk boot-scheduling deadlock — fixed and verified on the real
  worker; the first real emulator+game image booted to READY 2026-07-05.
- hypervisor M9 / capture engine / D7 framebuffer contract — all accepted;
  the harness `NoopPlatform` no-frame bug you fixed (`refwork-4qj`,
  `40eaf4f`) cleared the last frame-path defect, and the hypervisor's
  test-cap retune is filed against that repo separately.
- Your own image pipeline is real: pinned kernel 6.12.93, real
  `detguest-agent`, harness registers `wram`/`framebuffer`/`meta` before
  Ready, clean-root double-build byte-identical
  (`dist/workload-image-0.1.0/`).

What has NOT happened: the READY snapshot has not been regenerated from the
current (post-fix) image, no first-room in-VM run exists, and
`dist/workload-image-0.1.0/determinism.unstamped.yaml` still defers the
green stamp to "package 06" (and pins `git_rev: 84933d9`, already behind
`main`). Phase 3's exit gates 1 and 3
(`phase-3-workload-in-the-box.md`) are both yours to close, and Phase 4's
entry (real captures for the scorer corpus) waits directly behind them.

## The Ask In One Paragraph

Execute the remaining chain of your own plan: rebuild the package-04 image
from current `main` and regenerate the READY snapshot (`refwork-gp9`),
coordinate the operator cutover with us, run `refwork-verify vm-first-room`
end-to-end against the real worker (`refwork-d7t.11`), then run the M5
suite — double-run bit-identity plus mid-game snapshot/restore continuity,
20 consecutive times zero-flake, plus the `--nondet-test` negative — and
convert `determinism.unstamped.yaml` into the green stamp
(`refwork-d7t.12/.13/.14`), closing out with the CI real-worker legs and
guest-sdk handoff (`refwork-d7t.15`). Flag explicitly and early the
operator-input items no agent can supply: the ROM BLAKE3 / first-room
padlog BLAKE3 / run-owner lab fields.

## Files In This Request

| File | Contents |
|---|---|
| `01-current-state.md` | Evidence: what's green upstream, what's open here, the operator-gated items |
| `02-requested-work.md` | The chain, acceptance criteria, out of scope |
| `03-verification-offer.md` | The bridge's cutover procedure and first-room verification |
