# Native workflow review — Task 15.13

Status: source/concept review complete; final current-source platform and actual
spoken-reader receipts in progress. PR66 targets the Rust rewrite integration
branch. This record covers component workflows, with later qualification gates
kept explicit.

## Independent review and corrections

Two read-only reviewers inspected the complete change against repository standards
and the OpenSpec/parity register from integration base
`f62bab08e9f7db70c5b7b6cce8dbf69cbd121921`. The immutable Go oracle remains
`73a93303a9a44cf0a92eaf525331561ed8c04460`.

The full parity review identified four required composition corrections:

1. P42 lazy official artwork now reaches a joined, bounded native worker with
   coalesced requests, fallback, persisted cache and restart reuse. Cancellation
   reaches the actual HTTP and custody/staging token. Uncertain publication
   retains the actual metadata storage owners even if the jobs runtime fails.
2. P03 has separate Keep/Confirm foreign-schema recovery. Complete DB/WAL/SHM
   copies and a verified durable receipt precede removal of any original name.
   Actual process-death replay revalidates descriptors, identities and digests.
   Both normal and interrupted startup route unfinished receipts to explicit
   confirmation, including a compatible replacement. Actual SQLite open/close
   precedes completion. Failed work retains its actual native owner/resources.
3. P52/P53 incomplete native language preferences take precedence over legacy
   adoption language. The bound library reads the retained native profile choice.
   The native fixture adopts an unchanged FR legacy source in explicitly chosen
   EN and verifies EN again after restart.
4. P23/P56 batch Broken uses a truthful native Mixed/All/None choice with explicit
   Set/Clear, target scope and counts. Its mounted control survives completion and
   guarded Undo. Single-story controls remain Boolean.

The recovery regression also exposed an older setup/recovery snapshot overtaking
an offered language command. Generation-less observations now obey the same
accepted-revision boundary; the fixture waits for the actual requested revision.
Recovery completion retains the verified preservation paths in the activated UI.

The spoken harness review corrected false-positive review-close predicates,
offered-versus-accepted revision confusion and source/binary provenance gaps.
The resulting observer uses native keyboard input and checks actual accepted
states before positive worker shutdown. Accessibility queries only observe.

## Approved concept source and omissions

Both canonical saved concepts and their tracked copies were independently checked:

| Saved concept | SHA256 |
|---|---|
| Fragment | `44cd023b9e09e9bd2b411e76e58f5bef70e18b72508e03f25c8aba6343083460` |
| Standalone HTML | `4df3afb9a27c780657f3c4bf0f2b691f346bd00e589468516a5bc655230d2cb1` |

The review uses their saved source and hierarchy. The blocked preview URL was not
bypassed, and no live rendering claim is made for it. Toolkit widgets, typography
and window decorations retain the adaptations admitted in section14. Sidebar,
workspace/browser/inspector hierarchy, independent scrolling, intrinsic artwork
ratio, external captions, narrow French layouts and modal focus remain required.

The demonstrations omit representative requirements implemented by the native
surfaces: complete filters/tri-state choice/compatibility/sort/paging/history and
repair; independent inspection/bulk selection; inert descriptions and technical
metadata; full tag CRUD/protection/color/destructive impact; mixed Broken/exact
Undo; independent Collection unions/counts/copied queries; Shelf seeds/manual
membership/order/duplicates/unavailable reconnection/exclusions; explicit review
Keep/removal/stale/incomplete/source/detach/delete effects; real import/queue/
cancel/retry/duplicates; optional catalog/LKG/timestamps/hide-show/lazy artwork;
all export scopes/zero-ready/stale eligibility/original bytes/no-clobber/grouped
reports/reselection/reveal; durable History/dismissal; and actual setup/adoption/
override/relocation/recovery/diagnostics/native menus. HTML dialog simulations
supply design context; transaction, network and ownership proofs come from the
actual component, storage and fault fixtures.

## All parity rows

This matrix maps source and executable coverage. It does not replace platform
execution or broaden the later resource, handover, package and cutover gates.
`F` means `crates/librairii-desktop/examples/native-foundations`; storage evidence
includes the cumulative Rust gate and existing frozen-Go differential corpus.

| Row | Native surface and supporting coverage |
|---|---|
| P01 | Settings/root/override; native layout/identity/ownership; F maintenance. |
| P02 | Startup DB lifecycle; migrations/frozen-prefix differential/worker tests. |
| P03 | Separate Keep/Confirm; schema preservation/replay; F maintenance. |
| P04 | Import immutable managed copies, streaming hashes and original bytes. |
| P05 | Multiple-file picker/post-picker seam, archive corpus; F operations. |
| P06 | Import reasons/details; UUID/metadata/artwork/CRC parser fixtures. |
| P07 | Admission limits; traversal, malformed ZIP/7z and image hardening. |
| P08 | Duplicate/conflict reports; transactional publication/compensation. |
| P09 | Shared bounded import/sync/export scheduler; concurrent F operations. |
| P10 | Progress/reports; durable operation repository/restart reconciliation. |
| P11 | Actual cancellation/join/DB close; interrupted-operation/crash tests. |
| P12 | Native diagnostic Save; sanitized bounded no-clobber ZIP; F maintenance. |
| P13 | Library/workspace, three modes, inspector and bounded group previews. |
| P14 | Literal search; Unicode query differential; F filters. |
| P15 | Boolean/choice/language/status filters, summary/collapse/clear/sort/SQL. |
| P16 | Typed Library/Collection/Shelf paging, Back/Forward, persisted restore. |
| P17 | Collection repair, retained invalid criteria, query-version compatibility. |
| P18 | Independent inspection/bulk selection, frozen IDs/stale rejection. |
| P19 | Inert rich text/provenance/technical disclosure; F details. |
| P20 | Tag CRUD/order/color/destructive impact, storage tickets; F tags. |
| P21 | Mixed Boolean/additive choice, atomic frozen-target validation. |
| P22 | Protected labels/assignments, editable colors, restart/catalog preservation. |
| P23 | Detail/panel/bulk Broken, truthful mixed choice, exact guarded Undo. |
| P24 | Persistent warnings, unchanged filters/membership; F flags. |
| P25 | Collection CRUD/live counts/repair and independent Shelf navigation. |
| P26 | Union/count/overlap/copied-query flows and current-definition transactions. |
| P27 | Independent Shelf CRUD/manual membership/order; F shelves/curation. |
| P28 | Explicit seed approvals, source/manual union, cancellation/bounded pages. |
| P29 | Manual add/remove/order, exclusions, temporary refinement separation. |
| P30 | Unavailable UUID/provenance/order; removal/reimport reconnection. |
| P31 | Sources versus origins/manual Keep and ANY-source matching. |
| P32 | Separate review, Not now/Add/Exclude/Keep/Remove, explicit totals/Apply. |
| P33 | Incomplete coverage, healthy additions, suspended inferred removals. |
| P34 | Private freshness tickets, expiry/replay/stale refusal and atomic Apply. |
| P35 | Entry mismatch evaluation/dismissal/restart/new affected UUID callbacks. |
| P36 | Paged exclusions, Allow suggestions, exact review return. |
| P37 | Source/detach/Collection-delete impact, Keep-all and reviewed removal. |
| P38 | Duplication preserves decisions/provenance/order; reused-ID capability tests. |
| P39 | Explicit catalog sync/lazy artwork, qualified allowlisted HTTP adapter. |
| P40 | Staged atomic activation/LKG; real loopback failure/cancellation. |
| P41 | Resolution/facets/details refresh preserves manual organization. |
| P42 | Joined bounded lazy worker/cache/coalescing/cancel/retained custody. |
| P43 | Optional sync status/timestamp/Retry/Cancel/persistent Hide/Show. |
| P44 | Every export scope, complete query and UUID unions; F exports. |
| P45 | Frozen preview/reasons/counts, zero-ready disablement, destination reselection. |
| P46 | Prepared/queued Broken revalidation and committed-output preservation. |
| P47 | Original filenames/extensions/bytes; atomic verified no-clobber publication. |
| P48 | Byte/item progress, cancellation, grouped reports, validated native Reveal. |
| P49 | Separate frozen removal; durable trash/rollback; F details/recovery. |
| P50 | Twenty-card cursor History, fresh details/outcomes/Retry/Reveal/precedence. |
| P51 | Native navigation/view/sync visibility/exact-ID dismissals; legacy bridge Task17. |
| P52 | Explicit setup/adoption/override and independent language restart; handover Task17. |
| P53 | Native/in-app Settings, buffered commands, editing menus, EN/FR projection. |
| P54 | Full preview/move/cancel/rebuild/cleanup and cross-volume crash harness. |
| P55 | Missing-root, same-UUID Locate, wrong-UUID refusal and portability fixtures. |
| P56 | Workflow keyboard/focus/modal/state/localization; spoken receipts below. |
| P57 | Direct Rust/Slint/native adapters; final package execution Task18. |
| P58 | Bounded workflow models; numerical performance/resource acceptance Task16. |
| P59 | Supported-target packaging/install/execution/checksums Task18. |
| P60 | Component composition; complete handover/rollback/package/cutover Tasks17–19. |

## Evidence acceptance

The full native sequence retains all original foundation assertions, 90-second
per-scale deadlines, both concepts, densities/modes, menu/modal checks and workflow
callbacks. Current development execution passes88 PNGs. The new schema and mixed
workflow states also receive actual shown screenshots in the spoken fixtures.

`scripts/verify-native-speech.py` binds actual English/French keyboard/Orca/PCM
receipts to the retained current-source Linux executable. Each utterance requires
its expected Orca output after the marker, over1,600 nonzero samples and a trailing
silent segment within15 seconds. Raw PCM/WAV, offsets, semantic snapshots, native
screenshots, accepted/offered revisions and positive worker joins are retained.
AT-SPI alone never satisfies this spoken gate. The owner waived VoiceOver execution
and Japanese/Chinese/Arabic IME-specific execution; valid UTF-8 input remains tested.

Final provider run, artifact digests, target receipts and spoken case totals will
be recorded after execution. No final acceptance is claimed by this interim text.
