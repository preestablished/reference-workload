# Step 5: Adoption In This Repo And Handback To The Bridge

Repo: **reference-workload** (plus the request-thread handback).

## 1. Adopt the fixed guest-sdk rev

- Land/verify all guest-sdk work (steps 01–04) on guest-sdk's main and
  note the final rev.
- Bump `image/guest-sdk.lock` `[guest_sdk] rev` to that rev (the
  request's `02-repro.md` calls this "a one-liner on our side"). Check
  whether any `[boot_contract]` fields in the lock need to change —
  they should NOT (the fix doesn't alter the boot contract;
  `ready_after` stays `regions-registered-and-start-sent`). If step 03's
  root cause DID force a contract change, stop and surface that to the
  operator before bumping.
- Rebuild: `cargo run -q --locked -p xtask -- image build` (sibling
  checkout must sit at the pinned rev). Record the new image hash from
  `dist/workload-image-0.1.0/`.

## 2. Local verification with the new image

- Re-run the probe (step 01 §3 commands): expect breadcrumbs → `Ready`
  → frames advancing → no `control socket closed`.
- Run the step-04 VM-tier test against the new dist image; record the
  command and output path.

## 3. Handback: `03-resolution.md` in the request directory

Write `.agents/requests/phase3-ready-not-emitted-real-worker/03-resolution.md`
per the series convention (see how prior request threads in
`.agents/requests/` structure theirs). It must contain:

- Root cause for each symptom (symptom 2: agent fd-3 socket lifetime;
  symptom 1: whatever step 03 confirmed), with commit SHAs in
  guest-sdk and (if any) this repo.
- The adoption commit here (lock bump + rebuilt image hash).
- Exact repro-verification evidence: probe output excerpt, VM-tier test
  run, commands used.
- What the bridge should now do: re-run `dh-m9-ready-handoff` on their
  scratch paths against the rebuilt image and confirm green looks like
  `02-repro.md` §"What Green Looks Like" item 2 — READY at icount
  ≈ 643 M, `region_count 3` / `manifest_generation 6`, state hash into
  the handoff's public summary.

Commit the resolution and all repo changes (git conventions: commit
finished, verified units; explain the why in bodies).

## 4. Close the loop

- The bridge answers with `04-verification.md` in the request dir; if
  their real-worker run fails, treat their dump as a fresh step-01
  input — the breadcrumbs are now permanent, so the dump will name the
  leg directly.
- Downstream (for awareness, not this plan's scope): green here
  unblocks READY-snapshot regeneration (step 3),
  `BRIDGE_REAL_SNAPSHOT_REF` cutover (step 4), and the first real frame
  in the browser.
- Check `bd ready` / the bead graph for beads tracking this request and
  close them with `bd close <id> -r "..."` citing the resolution file.
  If no bead exists, don't invent one retroactively.

## Exit criteria

- Lock bumped, image rebuilt and hash recorded, probe + VM-tier test
  green locally.
- `03-resolution.md` committed; bridge pinged.
- Bridge's `04-verification.md` confirms the real-worker handoff
  snapshots READY.
