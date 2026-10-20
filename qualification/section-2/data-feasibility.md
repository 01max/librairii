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

2.10 remains **partial**: section1 populated migration/SQL/backup goldens and the
full accepted/rejected codec/solid/password/encoding corpus are not frozen here.
Need every accepted extension/content combination, required/unrelated payload CRC,
solid-block preceding-data cost, actual-read/dictionary/allocation caps, unknown
sizes/overflow and cancellation. `Read::take` bounds bytes delivered to this test;
it does not bound upstream header/dictionary allocation or implicit solid decode.
Inspect decoder allocation controls and account preceding solid entries before
untrusted-input qualification. Any baseline acceptance change requires its own
reviewed hardening record, never relabeling an unexplained mismatch as a pass.
Run matching native target tests before claiming I/L/W compatibility.
