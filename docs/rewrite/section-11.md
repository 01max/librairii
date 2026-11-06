# Task 11: optional Lunii metadata and artwork

Base: `59cb00dbea492d322f511fd6ea86e9164c636bde` (Task 10 merged, including
its later review corrections). One PR targets `rewrite-in-rust-native-desktop`;
11.1–11.8 each have a dedicated commit. Linux x86-64 and macOS ARM/Intel are the
admission targets. Windows stays deferred. Native sync/artwork screens remain
Task 15; this delivery supplies headless application/adapters/storage ports.

## 11.1 — HTTP boundary and dependency selection

Only the public guest-token, metadata catalog and referenced artwork protocol is
exposed. Requests use the frozen envelopes/headers. Each response has a 30 s total
deadline (the two-request refresh can take two such deadlines), 10 s connection
and 15 s stalled-read ceilings, no redirects/retries, and actual streaming caps of
64 KiB / 32 MiB. Errors omit response bodies, guest tokens and remote URLs. The
client admits at most two simultaneous logical requests and returns Busy without
an unbounded internal wait queue. No network operation runs merely by constructing
or opening a library. Tests use explicit numeric loopback endpoints and synthetic
TLS certificates, never public endpoints.

The design selects reqwest/Tokio/rustls. Exact pins are reqwest 0.13.5, Tokio
1.54.1, rustls 0.23.46, rustls-platform-verifier 0.7.1 and mime 0.3.17. Defaults
are disabled; no compression, HTTP/3, cookies, blocking-client runtime or account
features are selected. Ring is the explicit per-client crypto provider; TLS 1.2
and 1.3 use the platform certificate verifier, with no global-provider mutation
or insecure certificate/hostname bypass. Its default crypto-provider setup is
not assumed from feature unification. See the official [reqwest builder API](https://docs.rs/reqwest/0.13.5/reqwest/struct.ClientBuilder.html)
and [platform verifier API](https://docs.rs/rustls-platform-verifier/0.7.1/rustls_platform_verifier/trait.BuilderVerifierExt.html).
URL/IDNA dependencies do not replace the frozen domain search/language codec.

License review: the additional ISC terms for ring, rustls-webpki and untrusted,
and BSD-3-Clause terms for subtle, are permissive redistribution licenses requiring
retained notices. They join the explicit allowed-license set; inventory generation
collects upstream notices across all three targets. No advisory is ignored. Ring
requires getrandom 0.2.17 alongside the existing tempfile/proptest 0.4.3 API. That
single exact duplicate has an owner/tracking record expiring 2027-01-21; broad
subtree suppression remains forbidden. Reassess it when the provider updates.

The HTTP/TLS tests cover exact request headers/raw bytes, redirects/status/MIME,
invalid JSON/token, chunked bodies exceeding both limits, cancellation/deadline,
untrusted roots, wrong server name and a valid locally trusted chain. The fixture
leaf key is intentionally public synthetic test material, never a production key.

## 11.2 — Validated catalogs and raw custody

Normalization retains the exact response bytes and SHA-256, checks complete UUIDs,
canonical locales, duplicate identities, availability and numeric bounds, and
selects requested locale, same-language locale, then another available locale.
Text keys and language tags reuse the frozen compatibility codec; no new Unicode
normalizer is introduced. Typed snapshot IDs preserve SQLite's positive i64 range.

New raw files use `catalog/raw-<sync UUID>.json`, a managed relative path already
accepted by the legacy repository (the old nested layout remains readable by
legacy code). The existing retained-descriptor stage/no-clobber publisher supplies
hashing, durability and substitution checks. Its receipt and custody live through
SQL acknowledgement. Uncertain outcomes retain recoverable bytes; no compensating
pathname deletion claims rollback. Normalized rows/artwork registrations are one
transaction, with schema-enforced immutable content and staged/active/superseded/
rejected transitions. Activation belongs to 11.3. Tests verify raw bytes/hash,
locale fallback, invalid data, staged rejection, immutability and reopen.

## 11.3 — Activation and explicit refresh

The application refresh session retains one repository borrow across fetch,
normalization, staging and reporting. It is driven on a blocking worker; the
caller performs the isolated HTTP fetch between begin/finish. Cancellation is
checked before staging and immediately before activation. Once entered, the
uncancelled writer transaction reprojects Age, supersedes the same-locale prior
snapshot, activates the new one, and persists the successful report together.
Failures reject only staged snapshots and durably finish the failed/cancelled
attempt, leaving the previous active snapshot available. An uncertain commit or
lost acknowledgement is reported as outcome uncertain and never converted into
an invented rollback/failure report. Tests inject projection drift to verify
rollback, and exercise invalid/offline/cancel plus cancellation inside activation.

## 11.4 — Display, freshness and preserved user decisions

The existing independent title/description/author/artwork resolver now consumes
real activated rows. Catalog status exposes never-synced/fresh/stale-cache with
last attempt, activation time and matched count; metadata locale is independent
of presentation language. A complete refresh test covers official title with
embedded description/artwork, publisher/language/provenance, same-locale
supersession and stale offline cache. It verifies Age assignment and a recolored
Age definition survive refresh alongside user assignments, Broken and approved
Shelf membership/order. No new display-time path access is introduced.

## 11.5 — Lazy registered artwork

Artwork requires an opaque registered SHA-256 identity. The isolated HTTP client
validates the original allowlisted URL before parsing, refuses redirects and
unsupported MIME/signature pairs, and streams at most 10 MiB. PNG/JPEG/WebP remain
supported for compatibility. A library-owned service coalesces misses under one
lock, rechecks the cache, and bounds waiting callers to 64. Cancelled work stops
before publishing; an entered durable cache write finishes acknowledgement.

New cache files retain `catalog/official/<opaque ID>/<content SHA>.<extension>` through
the existing no-clobber publisher. The path is stored in the unchanged legacy
schema. Reads support existing nested catalog paths through retained no-follow
directory descriptors, reject links/substitutions and verify recorded length,
MIME signature and content hash. Failed/uncertain outcomes preserve recoverable
bytes. Tests cover coalesced fetches, allowlist escapes, chunked overflow, MIME
mismatch, registration, offline reopen, corruption and symlink refusal.

## 11.6 — Decoded artwork and model epochs

Image 0.25.10 is selected with only PNG, JPEG and WebP features. Decoding runs
synchronously on a blocking worker, with at most two admitted decoders. Headers
are checked before raster allocation: dimensions at most 8192, at most 16 Mi
pixels and at most 128 MiB decoder output. The decoder's 128 MiB allocation limit
also constrains intermediates where supported (upstream documents this allocation
limit as best-effort; it is not a process RSS guarantee). Thumbnails are at most
256×256 RGBA, held in an LRU capped at 128 entries / 32 MiB.

Opaque per-cache generation tokens prevent old work from repopulating or
publishing after invalidation. The native composition must invalidate on the
successful refresh report and before root relocation; the Task 15 UI integration
will own those events. Cache instances belong to one open library. Resolution
tries official then embedded PNG, otherwise a placeholder. Offline cached bytes
and legacy embedded paths are read without HTTP. Tests cover all three decoders,
malformed/compressed dimension bombs, admission, LRU eviction, fallback and stale
results after generation replacement.

The image graph needs a second exact duplicate exception: png 0.18.1 directly
uses miniz_oxide 0.8.9 while flate2 1.1.10 uses 0.9.1. Its owned exception expires
2027-01-21 and must be reconsidered when png changes. No advisory suppression or
new license allowance is needed; cargo-deny passes with retained upstream notices.

## 11.7 — Frozen differential and local transport evidence

The committed metadata fixture manifest authenticates frozen sources, probes,
inputs, repeated byte-identical outputs and the preserved synthetic cover.
Normalization compares all fields and artwork identities; the real Go artwork
publisher/loader establishes signature/hash/media/size and the exact legacy
nested layout. Frozen metadata/artwork/Lunii suites pass offline with cached
modules. Rust transport cases use local HTTP/TLS only; no account, device,
archive, firmware, upload or production request is exercised.

The comparison corrected UTC timestamp parsing/formatting, fixed-struct field
case folding, null attribution elements, long image paths, slash artwork URLs,
WebP signature matching, and the nested artwork layout. Six explicit malformed
or ambiguous JSON cases document stricter native catalog validation rather than
claiming parity; see the fixture README. Raster bounds are an intentional native
resource policy required by 11.6. These differences are submitted to 11.8's
independent review. The complete preservation corpus remains unchanged.

## 11.8 — Independent review and corrections

Standards review found a P2 loss of partial file-creation ownership and a P3
stringly typed status boundary. Spec review found a P1 stranded-running refresh
after interruption. All three are corrected in a separate fix commit.

`MetadataStorage` now owns the actual failed creation/publication/cleanup and its
original custody. The existing stage/publisher uses a private borrowed-or-Arc
loan to the same filesystem authority, so a retained failure holds the real
Database/library/lease lifetime without a self-reference or fabricated handle.
Known unpublished stages can retire through the existing qualified mechanism;
unresolved partial allocations/publications stay in the operation owner and
fence the generation. Tests inject failure immediately after payload acquisition
through both catalog and artwork repositories, substitute the pathname, and
verify the original retained FD still refers to the original bytes. Shutdown
must drain this owner before Database close; it cannot clear the sticky native
uncertainty fence or declare clean closure.

Startup, under the exclusive Database generation and before reader/work intake,
rejects interrupted staged snapshots and records interrupted attempts while
retaining all files and active snapshots. Dropping a pre-activation refresh
session durably cancels it; activation/unknown outcomes are never converted to
abandon failures. Reopen tests demonstrate last-good preservation and a new
successful refresh. Persisted legacy failure messages retain their original
English text; UI consumers receive typed status/error identities for localization.
Unknown persisted error codes are retained explicitly.

The Spec reviewer accepted the six documented JSON restrictions as explicitly
classified validation differences, as required by the quality specification;
the extreme-number case remains identified as valid JSON with a compatibility
restriction. Native raster limits are separately required by 11.6. A subsequent
fresh dependency audit rejected newly yanked Tokio 1.54.0; 1.54.1 replaces that
exact pin, with no warning/advisory suppression.

### Standards review result

The reviewer rechecked `eddf4bc…6dbd14e` and confirmed both findings resolved:
actual partial ownership remains in `MetadataStorage`, and typed status/error
DTOs preserve unfamiliar error codes. The private FileLoan extension retains
existing borrowed contracts. No new actionable standards regression was found.

### Spec review result

The reviewer rechecked the same correction and confirmed interrupted refresh,
pre-activation abandonment, last-good preservation and reopen/retry behavior are
resolved. Activation and unknown outcomes cannot become invented cancellation
reports. Persisted legacy messages and independent localization remain compatible.
No unresolved or new actionable spec findings remain; the six JSON restrictions
retain their explicit classification.

Review totals: Standards 0 open (P2/P3 corrected); Spec 0 open (P1 corrected).

Validation: the full pinned `cargo xtask check` passed on macOS ARM at
`6dbd14e`, including 417 executed tests, cumulative discovery/preservation,
workspace/fuzz dependency policy and advisory checks, source/notice checks,
formatting/build/all-target Clippy and ownership doctests. Linux x86-64 and Intel
macOS require their native provider checks on the PR head; this host result does
not claim those targets or desktop/UI/package release acceptance. Windows remains
deferred. All frozen Go metadata/artwork/Lunii suites and repeated fixture captures
passed using synthetic data/local transports with module downloads disabled.

### PR 52: native publication settlement

A successful raw catalog/artwork publication previously left its private stage
directory behind, preventing native finalization and the next checked reopen.
After a known SQL acknowledgement, the metadata owner now removes only the empty
owned stage directory, plus the separately owned unused payload when reusing an
existing destination. Descriptor/name checks and durable directory flushes remain
under the original custody; published catalog and cached artwork bytes remain.

Uncertain commits or lost acknowledgements retain their original receipts without
cleanup. Cleanup failures retain the actual publication/file/directory handles
and custody, fence the generation and preserve the already acknowledged SQL
outcome. Regression coverage exercises native refresh, new/reused artwork, clean
close and offline reopen, substituted stage/payload names, cleanup flush failure
and known versus uncertain acknowledgements. Existing discovery identities are
unchanged; the five regression tests are additive.

Validation: `cargo xtask check` passes on macOS ARM with 422 executed tests,
ownership doctests, formatting, all-target Clippy, cumulative source/preservation
checks and workspace/fuzz dependency/advisory policy. Other native hosts remain
unverified by this local run.
