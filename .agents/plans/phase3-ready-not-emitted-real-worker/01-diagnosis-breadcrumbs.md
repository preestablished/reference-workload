# Step 1: Bisect The Real-Worker Wedge With Boot-Leg Breadcrumbs

Goal: one instrumented real-worker run that names the exact leg where
the agent stops making progress. Everything in `03-fix-ready-emission.md`
keys off this step's output.

## Why breadcrumbs (and why they'll be visible)

The determinism-hypervisor's M9 handoff (`44c44f5`) already dumps all
buffered ring-A guest events on a non-Ready stop — that dump is our
display. Agent `LogLine` events (stream `log_stream::AGENT`) land in
ring A like everything else, so a breadcrumb emitted just before the
wedge will appear as the last event in the dump.

## 1. Add breadcrumbs to the agent boot leg (guest-sdk)

In `crates/detguest-agent`, emit a short, distinct `LogLine`
(`level` such that the default `log_mask 0x1F` passes it, stream
`AGENT`, via `channel.emit(...)` — droppable is fine, ring A is nearly
empty here) at each boundary of the wedge window:

- `control.rs::drive_refwork_start`: after HelloAck validated; after
  GameLoaded received; after harness-Ready received; after Start sent.
  (`drive_refwork_start` has no channel access today — pass one in, or
  emit from the call site in `runtime.rs` via a progress callback; keep
  the seam test-friendly.)
- `runtime.rs::autostart_and_ready`: after `drive_refwork_start`
  returns; after the `remove_file(GAME_IMG_PATH)`; after
  `wait_for_expected_regions` returns; after
  `emit_expected_region_evidence` returns (i.e. immediately before
  `emit_ready`).

Suggested texts (keep them grep-able and ≤ 32 bytes):
`boot: helloack`, `boot: gameloaded`, `boot: rw-ready`,
`boot: start-sent`, `boot: game-unlinked`, `boot: regions-gated`,
`boot: evidence-done`.

Also make the two spin loops fail inside the observation window instead
of outside it — see the hardening item in `03-fix-ready-emission.md`
§"Wedge-to-fault hardening"; you may pull that item forward into this
step, since a loop that boot-faults with a counter in the detail is
itself the best breadcrumb.

Keep the breadcrumbs behind clear code (they are cheap, deterministic,
and worth keeping permanently — do not plan to revert them; they emit
maybe eight small events per boot).

## 2. Check the ring-A drop counters in the dump

`channel.rs::bump_drop_counters` maintains
`OFF_RING_A_DROPPED_RECORDS/BYTES` in the channel header. Confirm the
M9 dump (or the bridge, when they run it) reports these. If nonzero on
the failing run, hypothesis H4 in step 03 (events dropped / criticality
misclassification) jumps to the top. If the dump tool doesn't print
them, ask the bridge session to add that to the dump — one header read.

## 3. Run the probe locally first

Per `02-repro.md` in the request:

```sh
cd ~/git/preestablished/reference-workload && cargo run -q --locked -p xtask -- image build
cd ~/git/preestablished/guest-sdk
zstd -qf -d -o /tmp/init.cpio \
  ~/git/preestablished/reference-workload/dist/workload-image-0.1.0/initramfs.cpio.zst
head -c 32768 /dev/zero > /tmp/game.img
BOOT_PROBE_INITRAMFS=/tmp/init.cpio BOOT_PROBE_GAME=/tmp/game.img \
  BOOT_PROBE_SECS=60 cargo test -p detguest-vmtest --test boot_probe -- --nocapture
```

(Note the image build verifies the sibling guest-sdk rev against
`image/guest-sdk.lock` — while iterating on uncommitted guest-sdk
changes you may need to build the agent + cpio by hand or temporarily
point the lock at your WIP rev; see how `xtask image build` stages the
agent before choosing.)

Expected: all breadcrumbs through `boot: evidence-done`, then `Ready`,
then (until step 02 lands) the frame-loop `control socket closed` death.
While you're here, **count the `NameIntern`/`RegionRegister` pairs** in
the probe's event dump: a correct boot should show 6 (3 registration-
time + 3 evidence-loop). Record the observed count — it double-checks
the wedge-window reasoning in `00-overview.md`, and the duplicate
emission is worth a note to the bridge if the handoff counts these
events.

## 4. Hand a candidate build to the bridge for the real-worker run

Write the current state into the request's `03-resolution.md`
(convention per `02-repro.md` §Handback) marking it explicitly as a
**diagnosis build**, with the guest-sdk WIP rev and what breadcrumb
sequence you expect. The bridge re-runs `dh-m9-ready-handoff` on their
scratch paths and returns the dump.

## 5. Decision table

Read the LAST breadcrumb in the real-worker dump:

| Last breadcrumb | Wedge is in | Go to |
|---|---|---|
| none after RegionRegister meta | fd-3 GameLoaded/Ready recv loop (agent) or harness never sent — split further: `boot: gameloaded` present? | 03 H1 |
| `boot: gameloaded` | harness-Ready leg: harness wedged between SDK meta reply and fd-3 `Ready` send, or datagram lost | 03 H1 (harness-side variant) |
| `boot: rw-ready` / `boot: start-sent` | `remove_file` or entry to region gate | 03 H2 |
| `boot: game-unlinked` | `wait_for_expected_regions` never satisfied | 03 H2 |
| `boot: regions-gated` | evidence pass (`copy_manifest_stable` seqlock or "manifest changed" path) | 03 H3 |
| `boot: evidence-done` but no Ready | `emit_ready` itself (ring full + critical spin, or doorbell exit mishandled) — check drop counters and consumer index | 03 H4 |
| all breadcrumbs AND Ready present | symptom 1 was fixed by instrumentation-adjacent change — suspicious; re-run un-instrumented before believing it | 03, re-rank |

## Exit criteria

- Breadcrumbs merged in guest-sdk with tests (a unit test asserting the
  breadcrumb sequence for a happy scripted boot leg is enough — see the
  `runtime.rs` tests' `ring_a_payloads` helper for how to read events
  back from a test channel).
- Probe run recorded (breadcrumb sequence + pair count).
- Real-worker dump obtained from the bridge; last breadcrumb identified;
  hypothesis selected in `03-fix-ready-emission.md`.
