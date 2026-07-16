## Action Items

### Critical
- None

### Important
- [ ] [crates/refwork-harness/src/frame.rs:130] Include the concrete `Core::fault()` detail in `EmuHalt` fault reports instead of the generic `"core returned FAULTED"` string.

### Suggestions
- [ ] [crates/refwork-harness/src/frame.rs:439] Add steady-state malformed and oversize datagram tests that assert `BadProto` faults and meta faulting.
- [ ] [crates/refwork-harness/src/frame.rs:298] Extract duplicated `bounded_fault_detail` logic shared with setup into one helper.
