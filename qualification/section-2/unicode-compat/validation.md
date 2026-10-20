# Current-base qualification checks

Executed on ARM macOS15.7.7/M4 Pro, Rust1.99.1, after history-preserving merge
e4e110078921841301024101e4f32048c2d97667 of admitted integration4383373. Other
native targets and physical/AT/toolkit qualification remain unverified. Later
documentation/ledger updates do not relabel the three measurement source maps.

| Scope | Actual result |
| --- | --- |
| Root `cargo +1.99.1 xtask check` | PASS: admitted146-file integrity/semantic witnesses, strict Python/discovery, source/dependency policies, root/fuzz deny/audit, locked native build, fmt, Clippy warnings denied,24 nextest tests and2 separate doctests. Root excludes all three nested qualification graphs. Existing unmatched NCSA allowance warning is retained; no deny policy changed. |
| Unicode standalone graph | Locked native build, fmt and all-target Clippy warnings denied PASS; nextest6/6,0 skipped; separate doctests0. |
| Prior spike, default | Nextest7/7,0 skipped; separate doctests0; fmt and all-target Clippy warnings denied PASS. |
| Prior spike, `gui` | Nextest7/7,0 skipped; separate doctests0; all-target Clippy warnings denied PASS. Actual widget debug/release Space→Apply→unchecked twice PASS; debug controller smoke PASS. Public in-process APIs only, no OS input/window or AT observation. |
| Decoder child, default | Nextest2/2,0 skipped; fmt and all-target Clippy warnings denied PASS. Binary-only child has no library doctest target. No hostile/control/container experiment rerun. |
| Python |13/13 exact identities PASS; no skip, expected failure or unexpected success. Unicode4 and existing supervision3 also PASS under `-O`. Safe mocked runtime boundaries only. |

`validation-identities.json` retains additive exact identities/features/target for
all four nested Rust combinations and13 Python tests. The Unicode nextest JSON
is retained verbatim in `nextest-identities.json`. All nested invocations use
explicit actual-host target and the root's explicit `.config/nextest.toml` with
profile `ci`, retries0 and fail-fast false. No all-features claim or GUI compile
substitutes for widget execution. Root CI/discovery does not automatically test,
audit or inventory these nested graphs; any later integration belongs to owner5.

Two tooling failures remain attributed separately. The first root nextest run
under the restricted sandbox could not bind its synthetic loopback fixture
(23 pass/1 permission failure); the authorized run with loopback available and
the subsequent current-base xtask run both pass24/24. The first explicit-target
GUI build failed when unchanged skia-bindings0.153.3 build.rs attempted its pinned
upstream cache/source download despite Cargo `--offline`; the network-enabled
retry passed. No network-hermeticity claim follows. Release widget build retains
the existing rust-objcopy/libLLVM strip warning with build/execution exit0.

The new text experiment uses only bounded synthetic/admitted text and verified
disposable oracle clones. Original archive/corpus/resource/Zstd report bytes are
unchanged;24 SQL/service-effect differences and six independent producer/reader
filename observations remain. The frozen146-file corpus, permanent436-file
oracle, external OpenSpec and product/root source were not directly edited by
section2; root repair files arrive only through the admitted integration merge.

Existing abdb318 native approval and eight CI successes are historical. Fresh
native review and current provider CI are required for the new published head.
No frontend baseline rerun solely for this text scope, physical platform pass,
language port, product graph/license/toolkit selection or merge is claimed.

## Required module-source correction after fd3a975 review

Native a8462328 requested correction4328610509. Separate fix52fc06d authenticates
the ZIP snapshot and materializes only its regular members, then verifies the
owned tree before build; download metadata is derived locally from authenticated
bytes/version. The actual-main extra.go test failed before the correction in
normal Python and `-O`, reaching the mocked Go-build boundary with an unverified
source. Afterwards it passes in both modes; another safe test covers bad h1,
symlink members and path traversal refusal before materialization. No actual
cache/oracle is mutated by the regression and no runtime command executes in it.

Focused verification:15 exact Python identities pass,0 skip/xfail/unexpected;
Unicode6 and existing supervision3 also pass optimized. Rust6/6, separate
doctests0, fmt and all-target Clippy warnings denied pass. Prior7/default+7/gui,
child2 and root24+2 identities/assertions remain present; those unchanged code
results above retain their original run attribution, not new final-head approval.
`validation-identities.json` adds the two Python identities without removing any.

`materialization-check.json` SHA256490227a1defe3d2815138904ccf5d074d03d910a9f8f22b9140197d3bc7cf305
records a real frozen Go1.26.5 build/provenance-only check using487 authenticated
ZIP members in a new independent disposable clone. All436 original files verify
before/after in both clones; the only clone addition is the qualification template.
No scalar/sequence/language outcome is remeasured; all three historical Unicode
reports and prior corpus/decoder/Zstd inputs remain unchanged. No original cache
poisoning is inferred. Review a8462328 applies to fd3a975 and requests this fix;
fresh final-head native review and all current CI remain required.
