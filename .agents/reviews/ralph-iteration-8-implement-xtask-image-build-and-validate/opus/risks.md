# Risks

- The validator has good coverage for the generated happy path but currently does not function as a strict package-04 contract gate. The biggest residual risk is accepting bundles that downstream boot tooling cannot consume.
- Artifact paths are taken directly from `workload-image.yaml` and joined to the manifest directory. Besides accepting renamed artifacts, this also leaves room for relative path drift unless exact file names or path normalization rules are enforced.
- `validate_no_game_content` scans visible files by name/extension, but it does not inspect compressed initramfs contents. If a wrong `--agent-bin` payload is supplied, validation has no way to detect that the packaged `/sbin/detguest-agent` is not the expected guest agent or contains forbidden game-like bytes.
- The external `zstd` command is an undeclared host dependency. Even if it is deterministic on one machine, the package hash is not reproducibly tied to `Cargo.lock`.
- The current tests prove selected negative cases, but they do not exercise the validation bypasses listed in `findings.md`. Adding regression tests for renamed artifacts, wrong device fields, and invalid sidecar TOML would close that gap.
