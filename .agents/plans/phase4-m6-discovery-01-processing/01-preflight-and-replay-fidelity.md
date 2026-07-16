# Package 01 — Preflight & Replay-Fidelity Gate (Start Here, Agent-Only)

## Goal

Prove the `discovery-01` session is intact and that **scripted replay of
`interactive.padlog` reproduces the hand-played WRAM byte-for-byte**. Every
later agent-only step (derived dumps, watch confirmation, map-check
expectations) rests on this gate: if replay is not byte-exact, downstream
discovery would chase phantom diffs.

## Steps

### 1. Resolve and persist the private root

The private root is supplied out-of-band by the orchestrating session (it is
never written into this plan). Persist it once, outside every checkout, so
later packages and the gate checker resolve it without re-asking:

```sh
mkdir -p ~/.agents/projects/reference-workload
# One line, absolute path, no trailing slash. Write it ONCE, by hand,
# from the orchestrator-supplied value:
#   printf '%s\n' '<private-root>' > ~/.agents/projects/reference-workload/private-root.path
chmod 600 ~/.agents/projects/reference-workload/private-root.path
PR="$(cat ~/.agents/projects/reference-workload/private-root.path)"
SESS="$PR/ramdiff/discovery-01"
```

If the pointer file does not exist and the value is not otherwise known to
this session: **stop** — that is the only permitted operator question outside
the marked STOP gates.

### 2. Session integrity checks

```sh
[ -f "$SESS/session.yaml" ] && [ -f "$SESS/interactive.padlog" ] || echo "FAIL: session incomplete"
grep -c '^- label:' "$SESS/session.yaml"        # expect 16
grep '^log_frames:' "$SESS/session.yaml"        # expect 45230
wc -l < "$SESS/interactive.padlog"              # expect 45231 (header + 45230)
for f in "$SESS"/*.bin; do wc -c < "$f"; done | sort -u   # expect exactly: 131072
```

Also confirm `candidates:` in `session.yaml` shows `offsets: []` (analysis
not yet run — if non-empty, someone already searched against the ORIGINAL
session; stop and reconcile, because package 03 requires the original to stay
pristine and all searching to happen in scratch sessions).

Tighten permissions per fast-follow 01's privacy posture: `chmod -R go-rwx "$PR"`.

### 2b. Repo-wide tracked-file privacy scan (pre-existing leak check)

Package 08's redaction audit only ever covered files this plan touches — it
can never catch a leak that was already committed. Scan every tracked file
for the private-root path component (derive the pattern from the pointer
file's value — its distinctive directory component(s); never write it into
this plan or any command that gets recorded publicly):

```sh
cd "$(git rev-parse --show-toplevel)"
git ls-files -z | xargs -0 rg -l "$(basename "$PR")"   # adjust the component per the pointer value
```

**Expected result: exactly one hit — `tools/record-ramdiff`** (lines ~41–42
hardcode the private-root location for ROM_DIR/SESSION_DIR; landed in commit
`5b35113`, already on `origin/main` — a pre-existing GATE-RECORD-ASK1
violation, see 00-overview privacy conventions). Handling:

- The working-tree fix (parameterize the tool via the step-1 pointer file) is
  package 02's job — do not fix it here, just confirm the hit.
- The already-pushed history occurrence is an operator decision item carried
  to STOP #1 (package 05 step 2). Never rewrite pushed history unilaterally.
- **Any OTHER hit is new data**: record it privately and resolve it before
  proceeding — do not continue with an unexplained tracked-file leak.

### 3. Builds and prerequisites

```sh
cd "$(git rev-parse --show-toplevel)"   # the reference-workload checkout
cargo build --release --locked -p ramdiff -p refwork-verify
python3 -c 'import yaml' 2>/dev/null || python3 -m pip install --user pyyaml
python3 -c 'import yaml' || echo "FAIL: pipeline layout stage needs pyyaml"
command -v b3sum jq bd rg
```

(`rg` is needed by step 2b's repo-wide privacy scan and package 08's
re-scan.)

pyyaml fallback (PEP 668): on an externally-managed Python, `pip install
--user` fails with an `externally-managed-environment` error. Fallback order:
(1) `brew install pyyaml` (or the platform package for the same interpreter);
(2) create a venv (`python3 -m venv ~/.venvs/refwork && ~/.venvs/refwork/bin/pip
install pyyaml`) and ensure that venv's `python3` is first on `PATH` for every
pipeline invocation that needs the layout-review step; (3) last resort,
`python3 -m pip install --user --break-system-packages pyyaml`. Whichever
route, the acceptance check is the same: `python3 -c 'import yaml'` exits 0
in the environment the pipeline will actually run under.

Notes: the interactive feature is NOT needed (no hand input anywhere in this
plan until STOP #1). `refwork-featuremap` has no release binary; the pipeline
falls back to `cargo run` for it — acceptable, or build it too.

### 4. ROM identity (private evidence only)

```sh
mkdir -p "$PR/evidence"
ls "$PR/ROMs" > "$PR/evidence/rom-listing.txt"          # never to stdout
ROM="$(find "$PR/ROMs" -type f | head -n1)"             # if >1 file, pick per operator's session note
b3sum "$ROM" >> "$PR/evidence/rom-identity.txt"
```

This session's padlog carries no `rom=` header (verified), so the replay gate
in step 5 is the authoritative "right ROM" test.

### 5. The replay-fidelity gate

Replay the full padlog with `--mark` at each of the 16 captured frames, into
a fresh sibling session:

```sh
RV="$PR/ramdiff/replay-verify-01"
target/release/ramdiff record \
  --rom "$ROM" --script "$SESS/interactive.padlog" --session "$RV" \
  --mark 1242=f1242  --mark 1539=f1539  --mark 3800=f3800  --mark 5230=f5230 \
  --mark 5375=f5375  --mark 12730=f12730 --mark 14452=f14452 --mark 19276=f19276 \
  --mark 23130=f23130 --mark 24358=f24358 --mark 29241=f29241 --mark 30101=f30101 \
  --mark 37080=f37080 --mark 39085=f39085 --mark 39966=f39966 --mark 41511=f41511 \
  --frames 45230
```

Watch stderr for `record: fault at frame ...` — any fault is a hard failure.

Byte-compare each replayed dump against the original (map frame → original
file from the session's own `session.yaml`; never hardcode the private label
filenames):

```sh
awk '/^  frame: /{f=$2} /^  file: /{print f, $2}' "$SESS/session.yaml" |
while read -r frame orig; do
  if cmp -s "$RV/f${frame}.bin" "$SESS/$orig"; then
    echo "frame $frame: IDENTICAL"
  else
    echo "frame $frame: DIVERGED"
  fi
done | tee "$PR/evidence/replay-fidelity-01.txt"
```

(If the replayed dump filenames differ from `f<N>.bin`, read `$RV/session.yaml`
for the actual names — see grounding note 3 in `00-overview.md`.)

### 6. Optional extra determinism evidence

```sh
target/release/refwork-verify double-run --rom "$ROM" \
  --script "$SESS/interactive.padlog" --frames 45230 \
  --report "$PR/evidence/double-run-45230.json"
```

Cheap, and gives a durable double-run report over the exact trajectory.

## Acceptance criteria

- 16/16 `IDENTICAL` in `replay-fidelity-01.txt`; zero replay faults.
- All step-2 integrity numbers exact (16 / 45230 / 45231 / 131072).
- `python3 -c 'import yaml'` passes; both release binaries exist.
- Private-root pointer file exists with mode 600.
- Step-2b repo-wide scan run; the only tracked-file hit is the known
  `tools/record-ramdiff` occurrence (until package 02 removes it); any other
  hit was investigated and resolved.

## On failure

- **Any `DIVERGED` or fault:** first re-check the ROM choice (step 4 — if
  `$PR/ROMs` holds more than one file, try the other before anything else).
  If still divergent, bisect: re-run with `--frames <first-divergent-frame>`
  and marks on nearby frames to find the first bad dump. A genuine
  replay-vs-interactive divergence is a determinism defect (P0-class per the
  project's rules) or a build mismatch vs the build that recorded the
  session — stop this plan and escalate with the private evidence file;
  nothing downstream is trustworthy until this gate passes.
- **Integrity mismatch (missing dump / wrong size):** the session is
  incomplete — stop; this plan's premise ("capture session just completed")
  fails and the orchestrator must re-check the session record.
