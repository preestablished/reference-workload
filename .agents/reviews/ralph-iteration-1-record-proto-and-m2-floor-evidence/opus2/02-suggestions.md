# Suggestions

### S-1 - Make the "Current State Verified" SHA match local branch reality

Path and lines: `.agents/plans/guest-sdk-unblock-reference-workload/00-overview.md:27-40`

What/why: The overview says the current state was verified on `main` at `9afaa0a`, while the local `main` for this review is `8c21d5d3fc76c2ea16ab3f76ea168218b8ac4c63` and the branch head is `34efa457f7ba2a4403bb3e1e9dac89b7baafeda1`. If `9afaa0a` is intentionally the CI/source evidence SHA, label it that way.

Suggested snippet:

```markdown
Verified on 2026-06-21. Local review base `main` is
`8c21d5d3fc76c2ea16ab3f76ea168218b8ac4c63`; branch head is
`34efa457f7ba2a4403bb3e1e9dac89b7baafeda1`. Nightly cross-arch evidence cited
below was produced at ancestor/source SHA `9afaa0a69a3ea57ed4e10ff29a53b716b5559990`.
```

### S-2 - Record control-plane dirty state with the proto provenance

Path and lines: `.agents/plans/guest-sdk-unblock-reference-workload/m2-floor-evidence.md:28-37`

What/why: The note records the sibling `../control-plane` rev and build result. Add clean/dirty state so a future reader knows the path dependency was built from committed bytes only.

Suggested snippet:

```markdown
| Worktree state | clean (`git -C ../control-plane status --short` produced no output) |
```

### S-3 - Define the unstamped WorkloadImage shape without drifting from API.md

Path and lines: `.agents/plans/guest-sdk-unblock-reference-workload/04-image-handoff-assets.md:76-77`

What/why: "Initially empty or marked unstamped" leaves too much room for schema drift. API.md requires `determinism.last_green` to be present for schedulable images. If package 04 emits a pre-suite manifest, specify whether it is schema-valid but unschedulable, or keep the unstamped state in a sidecar.

Suggested snippet:

```markdown
- Include the API.md `determinism` block in a stable shape. Before package 06,
  emit an explicit unschedulable sidecar such as `determinism.unstamped.yaml`;
  do not invent alternate `workload-image.yaml` fields unless API.md is updated.
```
