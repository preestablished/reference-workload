# Action Items

## Action Items

### Critical

None.

### Important

- [ ] [crates/refwork-harness/src/regions.rs:81] Replace the production region allocation path with locked/populated page mappings before advertising descriptors as API.md 3.5 regions.
- [ ] [crates/refwork-harness/src/meta.rs:50] Write meta payload fields first and publish `status` last for ready/running/faulted transitions.

### Suggestions

- [ ] [crates/refwork-harness/src/regions.rs:138] Prevalidate all `emu_buffers` sizes before marking any region published.
- [ ] [crates/refwork-harness/src/lib.rs:11] Remove the stale comment that says the harness avoids a `refwork-emu` dependency.
