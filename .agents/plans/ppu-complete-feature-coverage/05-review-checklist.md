# Reviewer checklist

Two independent reviewers should evaluate this plan before implementation. One
should focus on SNES PPU correctness; the other should focus on repository
architecture, determinism, testability, and scope.

Reviewers should answer:

1. Does the plan implement the reported `$2133=$40` feature rather than ignore
   it?
2. Is every current `UnimplementedBgMode` / `UnimplementedPpuFeature` PPU path
   mapped to a behavioral implementation and test?
3. Are Mode 7 EXTBG palette/priority, direct color, and offset-per-tile semantics
   accurate enough to implement without guesswork?
4. Does the 256x224 projection policy preserve the public API and remain
   deterministic, and are its compromises explicit?
5. Could the proposed refactor regress modes 0/1, windows, color math, OBJ, or
   zero-allocation behavior without a test catching it?
6. Is the private replay safe and sufficient to prove the user-reported route?
7. Which recommendations are required before implementation, and which are
   optional accuracy follow-ups?

The primary agent records accepted/rejected findings in
`06-reviewed-plan.md`, including the reason for each rejection, then updates the
preceding plan files where accepted feedback changes implementation or
acceptance criteria.
