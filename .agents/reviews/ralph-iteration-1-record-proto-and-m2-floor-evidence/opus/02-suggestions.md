# Suggestions

### Distinguish local `main` from `origin/main` in provenance

Path: `.agents/plans/guest-sdk-unblock-reference-workload/00-overview.md:29`

What/why: The overview says the current state was verified on branch `main`, `HEAD` `9afaa0a`, while the requested review base `main` resolves locally to `8c21d5d` and `9afaa0a` is `origin/main`. The distinction matters because this branch is reviewed with `git diff main...HEAD`, and future readers should know which revision was actually used for the state survey.

Suggested snippet:

```md
Verified on 2026-06-21 against product baseline `origin/main` `9afaa0a`.
This review branch is based on local `main` `8c21d5d`, which adds bead metadata
on top of that product baseline.
```

### Record both run revision and evidence-note revision

Path: `.agents/plans/guest-sdk-unblock-reference-workload/m2-floor-evidence.md:25`

What/why: The local run context records only the starting repo rev used for the checks. Since the evidence note itself is added by the branch commit, adding the final evidence-note commit keeps the evidence trail clearer without changing the claim that the commands ran before doc-only changes.

Suggested snippet:

```md
| Checked repo rev | `8c21d5d3fc76c2ea16ab3f76ea168218b8ac4c63` |
| Evidence note rev | `34efa457f7ba2a4403bb3e1e9dac89b7baafeda1` |
```

### Quote the placeholder banner exactly

Path: `.agents/plans/guest-sdk-unblock-reference-workload/m2-floor-evidence.md:80`

What/why: The evidence note paraphrases the feature-map placeholder banner with `--`, while the file uses an em dash. This is minor, but exact quoting avoids needless ambiguity in evidence notes.

Suggested snippet:

```md
> PLACEHOLDER FILE — offsets shown here are NOT validated game addresses.
```
