# 01 - Preflight Gates And Privacy

## Purpose

Turn the remaining operator dependencies into an explicit, auditable launch
decision before code touches private game material.

## Steps

1. Confirm a clean understanding of the checkout without overwriting unrelated
   user changes:

   ```sh
   git status --short
   git rev-parse HEAD
   bd show refwork-gp9 refwork-d7t.1 refwork-d7t.11 refwork-d7t.12 refwork-d7t.13 refwork-d7t.14 refwork-d7t.15
   ```

   Record the source commit used to build the exporter and bundle. Do not
   require `.15` for corpus production, although it is currently closed.

2. Create separate beads for:

   - capture exporter implementation and synthetic tests;
   - private real-offset feature-map/scoring discovery and validation;
   - operator corpus production, validation, freeze, and handoff.

   Add dependency edges matching `00-overview.md`. Include the bead ids in the
   eventual resolution and close each with evidence, not merely a code SHA.

3. Verify the deployed worker revision before capture. Record an opaque build
   or deployment ref proving it includes hypervisor `c0337ab` or later. If that
   cannot be proven, configure segment-bounded Runs following the engine-proof
   note; never assume an unbounded pre-fix worker is safe. Do not design around
   concurrent capture beneath a live `RunWithFrameCapture` stream because that
   mode is unproven.

4. Prepare an operator checklist in private storage and obtain affirmative
   answers for all of these:

   - approved single ROM and ROM BLAKE3;
   - run owner;
   - approved worker/image/snapshot refs;
   - a hand-play trajectory through the first boss;
   - a credits or late-game goal-positive fixture;
   - ordinary/first-boss goal-negative examples;
   - padlog capture and whether a recent tail may be retained in the context
     fixture;
   - private storage location, access group/token owner, retention, retrieval
     path, and registry-unavailable fallback;
   - whether any game/revision metadata may appear in public notes, or whether
     all such metadata must remain private.

5. Resolve `refwork-d7t.1` by either closing it with its required durable
   operator evidence or recording the epic owner's explicit sign-off deferral.
   Technical Phase 3 closure is not a substitute for this decision.

6. Select and record exactly one execution branch:

   - **Full corpus:** the hand-play and goal fixture session is scheduled and
     approved. Continue through all packages.
   - **First-room fallback:** only if the operator/owner explicitly approves
     it. Record its limited claims, keep the scorer fulfillment partial, create
     a follow-on owner/task for the missing trajectory, and execute the explicit
     schema and validator path in `04a-first-room-fallback.md`. Do not weaken
     the existing full-bundle validator globally.
   - **Blocked:** if neither branch has approval, finish fixture-testable work
     only and stop before private production.

7. Establish the private root outside every source checkout with restrictive
   permissions. Run `phase4-private-intake` only after the single-ROM guard and
   approval are confirmed:

   ```sh
   cargo run --locked -p refwork-verify -- phase4-private-intake \
     --rom-dir "$HOME/ROMs/SNES" \
     --private-root "$PRIVATE_ROOT" \
     --operator-approved \
     --operator-metadata-policy "$OPERATOR_METADATA_POLICY"
   ```

   Do not echo ROM filenames, exact private paths, tokens, or private refs into
   shared logs.

## Exit Criteria

- Gate record identifies the source/image/worker provenance and selected path.
- `refwork-d7t.1` is closed or has explicit epic-owner deferral.
- Operator session, storage/access/retention, and publication disposition are
  recorded privately.
- Private root is outside git and contains the intake skeleton.
- Exporter/map/production beads exist with correct dependencies.

## Stop Conditions

- No operator approval or no approved execution branch.
- Worker is pre-`c0337ab` and bounded-run protection is not available.
- The proposed private root is within a checkout or broadly readable.
- Any step would copy private literals into a public plan, request, bead, or
  terminal transcript retained outside the approved lab boundary.
