# Task 5 independent review checkpoint

Review base: epic5fd74f06fe4031b37bf84822d218943ec9ed970d. Two independent read-only
reviewers assessed the cumulative implementation through73188b6 and the native
filename regression below. This is a failed qualification checkpoint, not Task5
completion. No exception has been accepted merely because its record exists.

## Standards

Two findings fromfb6e905 were resolved separately in0cc7ca1:

- **P2: custody lost during artwork preparation.** `publish_artwork` previously
  took the retained guard before a fallible directory open. A transient failure
  could leave compensation with no original guard, violating the retained-owner
  requirement in section-5.md. It now opens the files before taking custody.
- **P3: possible Mysterious Name (judgment).** Generalized `OfficialArtworkRoute`
  and `official` fields represented both embedded and official routes. They are
  now `ArtworkRoute` and `category`.

The delta review found no additional documented-standard or consequential
ownership violation. Reduced constructor budgets, bounded header preflight and
selected-block traversal grant no filesystem authority. This standards result
neither establishes format parity nor admits the dependency.

## Spec

Four qualification findings remain open:

- **Blocking actual-memory bound:** sevenz-turbo models Brotli as32MiB but uses
  `brotli::Decompressor::new`. Locked brotli-decompressor6.0.1 enables large-window
  mode; a non-last tiny metablock can allocate a1GiB ring before output/header
  caps intervene. Source locations: sevenz-turbo decoder.rs688/container.rs memory
  model; brotli-decompressor reader.rs259/state.rs510/decode.rs1899. No hostile
  allocation probe was run. The feature is now disabled in both locked graphs,
  preserving a failing Go-accepted ordinary fixture. A bounded implementation is
  required before the codec can be admitted.
- **Blocking valid-codec loss:** the retained `encoded-header-bcj2.7z` is accepted
  by Go, while the candidate's encoded-header path rejects multi-I/O coders.
  Body BCJ2 support does not satisfy complete format/codec parity. Its required
  Accepted result remains a failing assertion, not a hardening exception.
- **Structural-count evidence gap:** the10000-entry constructor field also caps
  packed streams/other counts. The aggregate-coder record does not cover more
  than10000 packed streams with fewer than10000 files/coders. Retain a separate
  bounded accepted Go counterexample before reviewing that changed category.
- **Zstd evidence gap:** its decoder additionally caps windows at128MiB. The
  global256MiB decoder-memory record does not establish parity or a reviewed
  exception for that stricter window clamp. Assess a safe independently produced
  frame, using containment if its allocation behavior is uncertain.

The reviewer verified39 fixture hashes/sizes and7 source hashes. The ancillary
PNG-CRC case was subsequently executed across the complete manifest and proved
accepted by both implementations; its provisional exception was removed. The
remaining10 additional hardening records and image-bound record stay pending.
The header method preflight is bounded, and nested encoded headers are rejected
by both implementations. Selected-block traversal and explicit cancellation
checks fix the earlier all-block traversal defect.

The previously missing filename regression now imports/query-inspects/closes/
reopens with original raw filename and managed-path SQL bytes. Linux uses invalid
UTF8 bytes; macOS uses Unicode/spaces/case. The macOS execution passes; Linux
execution remains CI evidence. Source archives and legacy database bytes remain
unchanged.

Standards:2 findings resolved, none newly open. Spec:2 codec/safety blockers and
2 evidence gaps remain; actual Brotli allocation is the strongest safety finding.
