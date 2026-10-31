# Task 5 independent review

Review base: epic 5fd74f06fe4031b37bf84822d218943ec9ed970d. Independent read-only
Standards and Spec reviewers assessed the cumulative implementation, followed by
focused re-review of every fix through decoder qualification commit 02f6ce4.
Both final reports approve the reviewed code and evidence with no open findings.
Required native CI remains execution evidence separate from these judgments.

## Standards: passed

All three findings are resolved:

- Custody was taken before fallible artwork-directory setup. Preparation now
  finishes while resources retain the original guard, so compensation preserves
  authority on failure (0cc7ca1).
- The generalized artwork route used misleading official-only names. Its route
  and category names now describe both embedded and official artwork (0cc7ca1).
- The modified Brotli source lacked an explicit modification notice. Both patched
  files now prominently identify the Librairii changes, satisfying the retained
  Apache license's modified-file notice requirement (02f6ce4).

The reviewer found no residual documented-standard, ownership or consequential
maintainability issue. The final source gate verifies the original published
176-file index, original archive/revision, exact two-file patch, both Cargo
overrides and source aliases. Six actual owned-copy tests reject unauthorized
changes, removals, extras, original-index self-repinning, duplicate metadata,
symlinks/hardlinks and override substitution. Dependency inventories distinguish
the patched tree hash from the published archive hash and retain exact notices.

## Spec: passed

The original two codec blockers and two evidence gaps are resolved:

- Standard Brotli is restored. The large-window marker is refused before creating
  Brotli state, bounding the previously unbounded ring allocation path. The exact
  optional envelope is read completely; one stream is decoded with no automatic
  concatenation. A direct streaming wrapper reports excess packed input on the
  completing read. Accepted normal/enveloped/encoded-header and rejected large-
  window/concatenated observations match the actual frozen Go inspector.
- Encoded-header BCJ2 uses the existing bounded graph decoder. Valid retained
  Go-accepted headers now pass; this supported behavior was never waived.
- A valid independent raw-block Zstd frame with a 256 MiB window is accepted by
  Go and refused by Rust's separate 128 MiB window clamp. Its own retained record
  explicitly justifies that stricter category.
- A valid independently tested 7zz archive has 3,000 files/coders and 12,000 packed
  streams. Actual Go accepts it; Rust refuses its separate 10,000 structural
  ceiling. Its own counterexample and record establish that changed category.

Patch review found two further preconditions missing from header graph admission:
aggregate fixed-codec memory estimates/unknown models and agreement between graph
packed inputs and PackInfo counts. Both now fail before decoder construction;
checked packed-source ranges also precede the graph decoder. Tiny actual-constructor
regressions verify refusal without a panic or hostile allocation. Re-review found
no residual concrete decoder, cancellation, memory or graph defect (02f6ce4).

## Individually accepted hardening

The independent Spec reviewer accepted all 12 archive records in
`crates/librairii-adapters/testdata/archive-parity/hardening.json`:

1. Selected 7z member CRC validation.
2. Preceding solid-member CRC validation.
3. Decoder memory estimate above 256 MiB.
4. Stored/end-header buffering above 16 MiB.
5. Decoded-header buffering above 16 MiB.
6. More than eight coders per block.
7. More than eight streams per coder.
8. More than 10,000 aggregate coders.
9. Unmodelled decoder memory in unused blocks.
10. Incomplete PNG framing.
11. Zstd windows above 128 MiB.
12. More than 10,000 aggregate packed streams.

The separately retained PNG dimension and pixel limits are also accepted. Every
record preserves its exact fixture hash, actual Go acceptance, expected Rust
outcome and specific rationale. The reviewer verified all 48 archive fixture
sizes/hashes and both image fixtures. No valid-codec loss is accepted as hardening.
The frozen preservation corpus and Go inspector remain unchanged.

## Headless slice and execution evidence

The independent reviewer accepted the implementation and test design for actual
native import→inspection→publication→SQL/query→close→reopen. Tests cover original
archive bytes, atomic conflict/duplicate handling, compensation, cancellation on
both sides of commitment and registered-stage crash recovery. Original filenames
and managed-path SQL bytes survive reopen: Linux exercises invalid UTF-8 names;
macOS exercises Unicode/spaces/case. Invalid UTF-8 display fields remain empty
while retained raw bytes preserve filesystem identity.

Final local macOS ARM evidence on the reviewed implementation:

- `cargo xtask check`: 509 tests passed, no skips; 19 doctests passed; fmt,
  all-target Clippy, audits of both locked graphs, source/notice inventories,
  cumulative discovery and preservation checks passed.
- `cargo xtask fuzz`: identity 10,345,449 inputs and import_boundaries 561,411
  inputs, 31 seconds each, peak RSS 283 MiB and 296 MiB. Import uses a 64 MiB
  decoder budget to reach standard Brotli's 32 MiB model, with 16 KiB input/header/
  metadata/artwork, 64 entries, 128 KiB expansion, two-second per-input timeout
  and 512 MiB process RSS ceilings. No crash, timeout or resource failure occurred.
- Focused archive suite: all 27 tests passed, including BCJ2/Brotli headers,
  fragmented source reads, strict windows and both constructor regressions.

PR #55 must also pass native Linux x86-64, macOS ARM/Intel and focused coverage/
benchmark/fuzz checks on the pushed commits before platform qualification is
reported. Windows, GUI integration and general release/adoption gates remain
outside Task 5. These reports do not claim product release qualification.
