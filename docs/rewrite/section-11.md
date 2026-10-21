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
1.54.0, rustls 0.23.46, rustls-platform-verifier 0.7.1 and mime 0.3.17. Defaults
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
