# Task 5: archive inspection and immutable import custody

Dedicated branch: `codex/rewrite-05-import`, based on epic
`5fd74f06fe4031b37bf84822d218943ec9ed970d`. The PR targets
`rewrite-in-rust-native-desktop`. Each subtask has its own commit. Linux x86-64
and macOS ARM/Intel are required; Windows remains deferred.

## 5.1 — Actual container dispatch

The adapter opens the supplied retained seekable source, builds an index through
existing core inventory policy, and dispatches from the original filename plus
actual regular root entries. ZIP/7z mismatches refuse. STUdio root content takes
precedence and is rejected with every `.pk` variant. Plain content takes
precedence over named v1/v2 packs. STUdio ZIP and STUdio 7z retain distinct labels.
An opened index is not a successful inspection or custody receipt.

Candidate readers are pinned to rawzip 0.5.2 (MIT), flate2 1.1.10
(MIT OR Apache-2.0; already present transitively), and sevenz-turbo 0.29.2
(Apache-2.0). rawzip streams the central directory into a fixed 256 KiB buffer,
so the application checks entries before retaining a complete index. The 7z
candidate is a maintained in-process fork with constructor-level header, count,
name, coder and memory controls. Defaults, encryption, compression-writing and
extraction utility features are disabled; optional baseline decoder codecs are
explicit. It does not invoke a user-installed helper or introduce a process.

The prior sevenz-rust2 0.24.1 experiment in PR #38 remains unqualified and is
not imported. The newer candidate's API is a reason to test it, not evidence of
complete qualification. Current provisional ceilings are 16 MiB encoded/decoded
headers and 256 MiB decoder state, in addition to the unchanged core limits.
These and upstream structural ceilings require individual hardening records,
retained counterexamples and independent review before import admission. Full
codec/solid/CRC/cancellation/resource qualification remains 5.2/5.9/5.10 work.

Evidence: four adapter tests over unchanged preservation archives cover the
accepted suffix/content matrix, STUdio `.pk` rejection, wrong container/suffix,
selected reads and actual metadata-byte overflow. Focused `cargo test --locked
-p librairii-adapters archives::` passed 4 tests (10 unrelated filtered);
all-target adapter Clippy with `-D warnings` and rustfmt pass. These are host
checks, not three-target qualification or completion of the whole import task.

## 5.2 — Bounded index and reads

Every observed entry goes through the frozen InventoryBuilder and unsigned-to-i64
size conversion before retention. This preserves path cleaning solely as an index
key, distinct raw names, duplicate normalized-key refusal, directory/link rules,
nested archive suffixes, overflow-safe counts/sizes and each family's existing
ratio denominator. ZIP symlinks and 7z special/reparse/anti entries cannot become
regular assets. ZIP encryption flags cannot enable implicit decryption. Caller
limits can reduce the frozen limits for testing but cannot increase them.

The 7z constructor receives limits before parsing and maps count/name/output
ceilings to typed validation codes. Actual selected reads use fixed chunks plus
one detection byte, fallible output reservations, cancellation between chunks and
source reads, and never return partial metadata on a read/CRC/size failure.
Read/seek cancellation uses a non-retryable I/O failure internally; using
Interrupted would make read_exact retry cancellation indefinitely.

Eight focused adapter tests pass, covering actual frozen hostile archives, raw
path aliases/invalid UTF-8/traversal/drives/backslashes/depth/length, duplicate
keys, nested suffixes, 7z links/ratios, lowered entry/expanded limits, exact actual
read boundaries and corrupt ZIP payload CRC. Test writers explicitly use verbatim
paths so their normalization cannot hide hostile names. fmt and all-target
adapter Clippy pass. New decoder constructor hardening/codec/resource evidence
still belongs to 5.9/5.10; these index tests alone do not qualify the reader.
