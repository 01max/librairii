# Unicode15 candidate qualification (2.1 follow-up)

**Partial qualification only. No production graph, language port, license or
toolkit selection.** Observed on ARM macOS15.7.7/M4 Pro with Rust1.99.1 and
Go1.26.5. Other native target executions are unverified.

This standalone workspace evaluates unicode-normalization=0.1.22 (defaults off,
std), unicode-general-category=0.6.0 (defaults off) and the exact1616-entry frozen
full-fold map. Its own lock fixes14 registry identities; actual ARM metadata
selects8, including tinyvec1.13.3 with alloc. The optional locked packages are
not described as executed. `dependencies.json`, `features.txt` and `notices/`
record checked source archives, published VCS/manifest/notice identities and
actual selected features. Neither candidate declares its minimum Rust version;
the measured successful compiler is1.99.1, not a new MSRV claim. The category
crate runs its published local build script; there is no network generator here.

## Experiment and results

`observations.json` records complete stream counts, per-metric comparison counts,
every difference, source/binary hashes, actual tool/data versions and timings.

| Scope | Actual result |
| --- | --- |
| All1,112,064 valid scalars | Zero differences for CCC, Mn, frozen Go15 IsSpace, Rust std IsSpace diagnostic, full folding, raw NFKD, stream-safe NFKD and both final search pipelines. |
|15,550 sequences |570 raw NFKD differences; zero stream-safe NFKD, full-fold or final-pipeline differences on this set. All570 records retained. |
| Admitted Unicode expectations |25 actual Go outputs match unchanged expected results. |
| Admitted saved-query inputs |12 name strings compared as normalization inputs only; no query/domain replay claimed. |

Source-derived inputs include57 CCC/Mn representatives,3,249 ordered pairs,
513 repeated-nonstarter and513 explicit-CGJ cases at0/1/29/30/31/32/59/60/61,
all11,172 modern Hangul Jamo combinations,16 Hangul boundaries, and25 whitespace
plus25 whitespace/mark cases. These are bounded synthetic probes, not new product
goldens. The aggregate input hash and deterministic producer are recorded; each
difference retains its case identity and exact input/result UTF8 hex.

Raw `.nfkd()` omits Go's stream-safe CGJ insertion. The separately measured
`.nfkd().stream_safe()` route matches this set. The final pipeline removes only
Mn after full folding/decomposition, so raw intermediate differences do not
necessarily survive; the intermediate differences are nevertheless not erased.
The final pipeline uses the complete frozen Go15 White_Space range triplets,
not a fixture whitelist or the current Rust whitespace table. All-scalar testing
also measures the latter separately as a diagnostic.

This set does not prove all strings. In particular, repeated equal-CCC long runs
and two-scalar CCC pairs do not exhaust mixed-CCC long sequences crossing stream
boundaries. No production normalization adapter is selected from these results.
The report intentionally exits2 for the570 observed intermediate differences.

### Mixed-class supplement

The baseline report and its exact measured source are preserved at commit
1728a13. `mixed-observations.json` records a separate896-case experiment, without
rerunning scalars or admitted inputs. For each selected CCC/Mn representative,
it pairs minimum/maximum CCC representatives in tail and alternating runs at
29/30/31/32. All896 inputs and1,568 difference records are retained:896 raw NFKD
and672 stream-safe NFKD differences. Full folding and both final pipelines match
on this set. Insertion/reordering around CGJ means `.nfkd().stream_safe()` is
**not an exact Go NFKD adapter**. The earlier zero is limited to the baseline set.
This supplement's pivots are Mn; mixed surviving non-Mn marks remain a specific
follow-up case rather than a claimed final-pipeline proof. Reproduce with the
same driver arguments plus `--suite mixed`. Each report has its own source map;
the older report is not relabelled as an execution of subsequent source edits.

## Oracle and source integrity

Before any execution the driver verifies immutable oracle commit73a93303/tree
e797e9d5, clean status and all436 tracked source lengths/hashes. It uses
`git clone --no-local --no-hardlinks` into its own temporary root, checks the same
identities there and refuses alternate object databases. Only that disposable
clone receives the Go template and executes the oracle. The persistent oracle
and all436 original clone files verify unchanged afterwards.

Go uses local1.26.5, CGO disabled, isolated writable build/module caches, offline
proxy/sumdb and readonly go.mod. The existing x/text ZIP's Go h1 checksum is
independently recomputed and all487 members compared with cache bytes before
copying sources. x/text0.43.0 selects Unicode15 tables under !go1.27; runtime
provenance asserts Go/cases/norm15 and languageCLDR32. No current Go replacement,
generator or production library/profile/endpoint is used.

The driver verifies every admitted146-file hash, then exports only unicode.json
and saved-queries.json from exact1cd2244 into owned temp files. It does not replay
profile/relocation or query codecs. Both earlier24 SQL/service-effect differences
and six independent filename observations remain unchanged elsewhere.

Both Rust execution and compiler provenance use explicit `rustup run1.99.1`
from this nested cwd and explicit actual-host target. Unexpected inherited target,
rustflags/rustdocflags/runner values refuse execution; only their presence is
recorded. Cargo target output is isolated in the owned temporary root. The
global default is checked unchanged before/after. Ancestor Cargo config still
contains only the xtask alias; no root/shared config or CI was edited.

## Reproduce the scoped experiment

Use the installed pinned tools/cache, not new provisioning. From the repository:

```sh
PATH=/private/tmp/librairii-section3-tools/bin:/opt/homebrew/opt/rustup/bin:$PATH \
CARGO_HOME=/private/tmp/librairii-4-cargo \
python3 -B qualification/section-2/unicode-compat/probe.py \
  --oracle /Users/maxime/.ao/data/oracles/librairii-73a93303 \
  --text-cache /Users/maxime/go/pkg/mod/golang.org/x/text@v0.43.0 \
  --go /opt/homebrew/Cellar/go/1.26.5/libexec/bin/go \
  --output /tmp/unicode-observations.json
```

The Go template is `.go.in` deliberately: ordinary current Go discovery does not
compile or run this experiment. Rust root checks also exclude this workspace.
No root CI success may substitute for running its tests. From this cwd, build,
fmt, warnings-denied all-target Clippy, locked nextest with the root's explicit
nextest config/ci profile, and separate doctests are required. Actual nextest
identities are in `nextest-identities.json` (5 tests, none ignored). Safe Python
supervision tests are `python3 -B -m unittest discover -s tests -p 'test_*.py'`.
Those tests run no Go/oracle/build; malformed protocol, missing records, drift,
escape, duplicate/oversized inputs and tooling failure must not become a pass.

## CLDR32 read-only closure

`language-closure.json` pins the conservative compilation closure of
x/text0.43.0/commitac40b0f06c159afb5bfca4d1860f8b4b60dbc160: internal/tag(1 file),
internal/language(10), internal/language/compact(5), language(7). All23 source/data
files have exact lengths/hashes and imports; standard-library behavior remains
the separately pinned Go1.26.5 toolchain. This is a complete package dependency
superset, not proof every function executed or a new Rust language port.

The relevant source path is public Parse(34) -> Default.Parse(46), then internal
language.Parse(230)/parseTag(300)/parseExtensions(455) and recognized/alias tables.
An error returns before canonicalization; the product rejects even a partial
usable result. Default is Deprecated|Legacy, not BCP47/All|CLDR. Canonicalize may
remake the stored string; makeTag goes through compact.Make. Public Tag.String
(language.go226) returns internal Tag.String (internal/language/language.go141),
including stored extension/variant text and generated core serialization. The
product wrapper separately trims, replaces underscores, enforces35 bytes, rejects
errors/Und and returns that exact String. Syntax-only acceptance, current aliases,
script suppression, maximization and matching are not interchangeable.

Selected Go/xtext source terms and PATENTS are captured byte-identically under
`notices/`; the derived White_Space table retains Go source attribution. Candidate
archive notices, licenses and source identities cover all14 locked packages and
are cross-referenced in `dependencies.json`. No product license or allowlist is
changed, and this inventory is not an advisory/audit/deny-policy pass. Historical
CLDR/IANA raw generator input archives are not contained in the module: the
translation source is the exact published Go source/generated tables, not a
regeneration from current network data. Owner5 retains source-policy, underlying
dataset notice decisions and the eventual exact6 root graph. No runtime Go,
production language adapter, fixture-alias whitelist or full CLDR32 qualification
is introduced here.

2.1 and2.13 remain partial. Physical/AT/licensing/packaging/toolkit gates and the
independent decoder constructor/resource gate remain open. No merge is authorized.
