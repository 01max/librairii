# SQLite and archive feasibility (2.10, P02–P07)

Observed 2026-10-20, librairii-4, ARM macOS15.7.7/M4 Pro, Rust1.99.1.
`cargo test --locked --no-default-features`: **2 controller + 5 data tests pass**;
separate doctest harness has zero examples. `data-fixtures.json` records exact
source migration and synthetic archive SHA-256 identities from oracle source
`73a93303a9a44cf0a92eaf525331561ed8c04460`. No fixture generator or product code was
changed. Every writable database is created under a unique temporary directory.

## Observed candidate capabilities

* rusqlite0.40.2 / libsqlite3-sys0.38.2 bundled **SQLite3.53.2**: all 18 empty
  SQL prefixes reopen and advance to018; identity/18-entry ledger, integrity,
  foreign-key check and JSON SQL pass. This is direct SQL feasibility, not a
  production migrator, populated golden fixture or Go differential comparison.
* Online backup includes committed WAL rows and remains independent of subsequent
  source edits; dropped transaction rolls back DDL. SQLite compile options include
  THREADSAFE=1, ENABLE_STAT4/FTS5, DEFAULT_FOREIGN_KEYS, MAX_VARIABLE_NUMBER32766.
  SQLite C includes ENABLE_LOAD_EXTENSION, but the Rust loading feature is absent
  and SQL `load_extension` is verified to fail as “not authorized”. Do not claim
  the C functionality was compiled out. Exact options print with `--nocapture`.
* zip9.0.5, defaults disabled, deflate-flate2-zlib-rs: Store and Deflate roundtrip
  Unicode member names and exact synthetic bytes; full read reports changed CRC;
  malformed container rejects and `enclosed_name` exposes traversal as unsafe.
  No archive is extracted. Product path/classification policy is not implemented.
* sevenz-rust2 0.24.1, AES/util/encoder defaults disabled; BZip2/PPMd/Deflate/Zstd/
  LZ4/Brotli decoder feature candidates enabled in tests. All seven checked-in
  synthetic **LZMA2** containers parse and decode to a 1 MiB test-only ceiling with
  one decoder thread; source bytes remain identical. Generic6 entries/90 bytes,
  STUdio4/428, bomb7/262234, deep-path7/110, symlink7/100, nested7/114, missing5/78.
  Malformed signature/truncation/header-CRC probes reject.

The bomb/deep-path/symlink/nested/missing cases are intentionally **not product
acceptance passes**. The decoder exposing their entries says nothing about
Librairii's policy rejection codes. The 1 MiB test ceiling is not a replacement
for frozen product resource limits. Enabled codecs compiling is not codec corpus
qualification. No upstream extraction utility or password/decryption path is used.
Zstd introduces native zstd-sys C; SQLite and Skia also retain C/C++ dependencies.

## Open evidence

2.10 remains **partial**: the populated corpus observations below do not qualify
the full codec/solid/password corpus or implement the admitted domain contract.
Need every accepted extension/content combination, required/unrelated payload CRC,
solid-block preceding-data cost, actual-read/dictionary/allocation caps, unknown
sizes/overflow and cancellation. `Read::take` bounds bytes delivered to this test;
it does not bound upstream header/dictionary allocation or implicit solid decode.
Inspect decoder allocation controls and account preceding solid entries before
untrusted-input qualification. Any baseline acceptance change requires its own
reviewed hardening record, never relabeling an unexplained mismatch as a pass.
Run matching native target tests before claiming I/L/W compatibility.

## Admitted PR39 corpus comparison

Observed 2026-10-20 by librairii-4 on the same ARM host. Input is exactly PR39
commit [`1cd22443cae3b548bb7f9b3fb2435dcd73c3c65c`](https://github.com/01max/librairii/commit/1cd22443cae3b548bb7f9b3fb2435dcd73c3c65c),
manifest SHA256 `a59f2194cf5c1ad317485a8714e703c12fe161bfad0190a67ba5816ea7f46e53`.
Section1 admitted the corrected146-file corpus through integration merge
`9a6c1c116d9df726a061ad6aac3955df4ed41263`. The earlier e7c771b report remains
preserved in87d6ecd history. Corrected input changes the representative database
byte hash/WAL header and relocation evidence; this rerun retains the same24 SQL
and six Unicode differences. Domain semantics have not been ported or compared.
No relocation/profile input is executed. Owner3 receives the reference and all
mismatches; section2 does not edit its files.

`probe-corpus.py` verifies all146 corpus file lengths/SHA256 against the exact
pinned manifest, exports87 selected Git blobs into a fresh temp directory, and
checks those bytes again after probing. It never checks out the other branch or
executes generators.
The nested Rust example opens source databases read-only and uses SQLite online
backup to create all writable copies. Its report, `corpus-observations.json`,
retains every exact mismatch and raw oracle result alongside candidate evidence.
The probe exits **2** for measured differences; that is not a passing parity gate.
With the optional native codec experiment it records30 differences:24 SQL fields
and six end-to-end Unicode filenames. No difference is normalized away.

* All 18 populated prefixes and the representative schema018 database match their
  complete before snapshots, including exact schema SQL, NULLs, row/array order,
  identities and foreign-key/integrity results. All 19 online backups match too.
* Applying only the unchanged SQL migrations matches `afterGoOpen` for prefixes
  001–010. Prefixes011–018 each differ in three Collection fields: the SQL-only
  `query_payload` retains Broken definitionId7, `query_version` stays1 rather than5,
  and `revision` stays1 rather than2. **24 differences remain**. The Go fixture
  producer also opens Collections through its service; this probe has no such
  application adapter. No payload/version/revision exception or rewrite is added.
* Fixture timestamp handling copies only the producer's explicit `fixTimes`
  columns and fixed value, retaining NULLs. It does not sort rows or rewrite
  queries, IDs, revision numbers, paths or schema text. Physical producer-byte
  hashes identify inputs; complete semantic snapshots compare different engines.
* All20 inspection cases are observed in frozen array order. Nineteen containers
  decode completely; `invalid.zip` fails container parsing. The eight Go-accepted
  inputs have matching diagnostic UUID candidates; all three expected artwork
  payload hashes match. ZIP raw name bytes, member order/duplicates, method,
  declared and computed CRC, sizes and path-safety signal remain visible.
* The other eleven Go-rejected cases also decode: missing entries, disguised or
  unsupported suffixes, unsafe paths, duplicate paths, nested archives, ratio and
  link policies are **not implemented** by this decoder probe. The exact Go
  rejection codes remain in the report. Decoder success cannot replace them.
  Titles/descriptions, artwork validity/media type and full classifier precedence
  are not compared. This is not eight product-acceptance passes.
* All seven frozen 7z archives report solid blocks with LZMA2 coder ID `21`.
  Entries are sequentially fully read with one decoder thread and a 1MiB total
  delivered-byte ceiling; block sizes/properties and CRC observations are recorded.
  This does not measure random-access preceding-data cost or cap decoder/header
  allocations. The fixtures are trusted synthetic data, not an untrusted-input API.

Run explicitly from the application checkout after fetching the pinned Git object
(the script deliberately does not fetch or silently substitute a newer corpus):

```sh
git fetch origin 1cd22443cae3b548bb7f9b3fb2435dcd73c3c65c
python3 -B qualification/section-2/probe-corpus.py --output /tmp/section2-observations.json
python3 -B -m unittest discover -s qualification/section-2/tests -p 'test_*.py'
```

Python3.11+ and rustup with the pinned toolchain installed are required. Both
execution and Cargo/rustc provenance use `rustup run` with the channel read from
this directory's rust-toolchain.toml and run from this directory. The caller's
global default is not changed. On this Homebrew host the rustup proxies are in
`/opt/homebrew/opt/rustup/bin`; they may need to be added to PATH.

The five Python regression tests protect exact identity/order differences,
missing-vs-NULL/type distinctions, Unicode forms, and rejected-suffix results
remaining partial, plus actual pinned Cargo/rustc execution under an older caller
toolchain override. The root invocation was also observed with global stable
rustc1.97.1 before/after; report provenance records actual1.99.1.
The Rust example uses dev-only [serde_json1.0.154](https://docs.rs/crate/serde_json/1.0.154),
already present in the lockfile, default `std`, MIT OR Apache-2.0, MSRV1.71;
[pinned upstream manifest](https://github.com/serde-rs/json/blob/v1.0.154/Cargo.toml).
This adds no GUI testing entitlement or production dependency selection.

## Independent local codec/CRC experiment

The optional `--bsdtar /usr/bin/bsdtar` adds native producer experiments to the
same report. Host tool observed: bsdtar3.5.3 / libarchive3.7.4, zlib1.2.12,
liblzma5.4.3, bz2lib1.0.8. No tool is installed or selected for product shipping.
Two synthetic files have fixed timestamps, names, owner fields and exact payload
hashes. The second name is Unicode; no OS IME/filename interaction is inferred.

COPY, LZMA1, LZMA2, BZip2, Deflate and PPMd decode both exact payload hashes in
order. **Each has a filename mismatch**: NFC `Été 日本語.bin` supplied through a
deterministic synthetic PAX tar becomes decomposed `Été 日本語.bin`. The report
retains both forms plus the native producer's own archive listing, to distinguish
producer/reader stages. No filesystem extraction occurs. Five compressed variants
report solid blocks; COPY is non-solid. This observation is not an admitted
filename compatibility exception or a candidate failure attribution by itself.
Mutating either the preceding or target COPY payload while retaining header CRCs
causes `ChecksumVerificationFailed`. These checks do not exercise every Go CRC
policy or establish cancellation/solid resource limits. The native bsdtar producer
rejects Zstd output as an unknown compression name; that producer limitation stays
in the report, separately from the independent Zstd experiment below.

`synthetic_7z.py` is an original minimal writer based on the upstream
[7z format reference at9128b80](https://github.com/ip7z/7zip/blob/9128b80e3a471108678e2b3ec3c985861c8d7b0f/DOC/7zFormat.txt),
reference SHA256 `c5aba210bcc35e0506fa06bed771cf1807cbd0098dad8916b3b65ae819f5b077`.
It uses existing CPython3.14.7 [compression.zstd](https://docs.python.org/3.14/library/compression.zstd.html)
with libzstd1.5.7, level3, independently of the candidate decoder. No upstream
fixture or writer implementation was copied. Tool terms are in the pinned
[CPython LICENSE](https://github.com/python/cpython/blob/v3.14.7/LICENSE)
(PSF license with historical notices) and libzstd
[BSD license](https://github.com/facebook/zstd/blob/v1.5.7/LICENSE) /
[GPLv2 alternative](https://github.com/facebook/zstd/blob/v1.5.7/COPYING).
This is local producer attribution, not a product distribution-license decision.

The retained original `fixtures/zstd-independent.7z` is217 bytes, SHA256
`54de17c1dcaef5862816409eda41ff4cc6e57a303ef98eb182b2aa2a42013d39`.
Repeated generation produces identical bytes on the recorded producer. Its solid
block (`04f71101`) decodes two members in order with exact names and payloads:
`preceding.bin`,37000 bytes, SHA256
`c85dadb5ce6664a59a9195f5e6e52202bff312b97f32dea5a44dd51b83929f75`, CRC3377173824;
`Été 日本語.bin`,1440 bytes, SHA256
`95ee0511d9fa5a61138080ceaa203154e420e930f6de8929dcfa1a0a75a30053`, CRC2971439831.
The public Rust example observes38440 delivered bytes and matching header CRCs;
the producer regression independently verifies start/next-header CRCs and ordered
payload hashes. This synthetic decoder capability supplies no Go acceptance oracle.
Regeneration against the admitted146-file corpus retains **24 SQL +6 filename
differences**, both COPY corruption errors, all19 backup matches and exact frozen
classification/order evidence. It exits2 as before; no full parity pass is claimed.

Still missing: product classification/metadata and
required-vs-unrelated payload error semantics; admitted non-LZMA2/solid/non-solid
cases, LZ4/Brotli and wider Zstd combinations, encryption/password policy, encoding/unknown-size/overflow,
corrupt solid blocks and cancellation; actual-read/dictionary/allocation controls;
production migration/backfill/rollback/recovery behavior and all other native OS
execution. Those gaps stay open without a compatibility exception.

Pinned-source control audit (not an adversarial allocation test):
[reader.rs0.24.1](https://docs.rs/crate/sevenz-rust2/0.24.1/source/src/reader.rs)
sets `MAX_MEM_LIMIT_KB = usize::MAX / 1024`, passes that constant to decoder
construction, and offers thread-count control without a public per-reader memory
budget in the inspected `ArchiveReader` API. `Archive::read` parses before the
entry callback; header bytes are checked against actual input length before
allocation, but declared file/coder counts also drive allocations. The pinned
[decoder.rs](https://docs.rs/crate/sevenz-rust2/0.24.1/source/src/decoder.rs)
checks LZMA2 and PPMd memory against that constant, while LZMA1 constructs its
dictionary from coder properties. This is insufficient evidence for product
dictionary/header/allocation ceilings. A bounded adapter/decoder strategy and
adversarial tests remain independent engineering work; do not expose this
trusted-fixture example as an untrusted-input API or call one thread a memory cap.
