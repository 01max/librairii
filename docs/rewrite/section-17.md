# Task17: Go bridge, native transfer, upgrade and rollback

Integration base: `1f2ef5940a5a684732a27ea4a4c1cf49f03c04ca`.
Branch `codex/rewrite-17-handover-rollback`; [PR68](https://github.com/01max/librairii/pull/68)
targets `rewrite-in-rust-native-desktop`. The
[transition and recovery guide](../native-transition.md) is the user procedure.

## Ownership and the supported route

The Go bridge takes the same nonblocking exclusive OS `flock` on the retained
regular single-link `.librairii-library.lock` inode as Rust. It acquires profile
and root ownership before bootstrap/recovery, schema repair, initialization or
root publication. Closing SQLite during relocation does not release ownership;
source and destination leases remain through durable jobs and final close.
An uncertain close permanently fences the session and retains its leases.
Independent language changes serialize short read/modify/write publication and
do not rewind the configured root or a newer saved language.

Lock controls are excluded from Go payload copying. Source cleanup never unlinks
the held control, including when an earlier journal lists it as payload. A fully
cleaned old root may retain a harmless directory/empty-lock shell. Changed or
foreign payloads remain pending. A reserved name with nonempty bytes, a symlink
or multiple hard links is refused as a setup control.

This cooperative protocol does not exclude unmodified older Go. In-place native
adoption remains unconditionally refused. Both configured and default legacy
locations use verified independent copying with fresh target inodes and native
profile/recovery authority. A closed bridge source may include its empty control
in the full source manifest; Rust verifies it but creates its own fresh lock.
The source control is never copied or replaced. Foreign journals/path strings
remain data, not cleanup/rebinding authority.

After Rust use, Go bridge opening of a native-owned root is explicitly refused
before recovery or SQLite as the supported rollback policy. On disposable copies,
actual Go reopening preserves the tested Collection query version5/payloads,
Shelves, memberships, outcomes, existing tags and native-edited story fields.
Go supports query version5. Its startup can refresh derived search names and add
derived Age tags, so raw snapshot inequality is recorded separately from query
compatibility. The rehearsal retains both full snapshots and classifies those
observed deltas; it does not qualify arbitrary native edits for direct rollback.
The supported rollback uses an independent pre-native source/snapshot, keeping
the newer candidate and its preferences intact for explicit reconciliation.

## Version1 preference protocol

Go builds a strict `librairii.preference-handover` document from actual library
identity and backend legacy-Broken compatibility facts. Caller state cannot set
the binding. It whitelists List/Small covers/Large covers, canonical operation
UUIDs, sync visibility, typed Library/custom/Collection/Shelf semantic frames,
known back/forward position and Shelf order choice. Equal numeric Collection and
Shelf IDs remain distinct. URL versions1–3 reuse the retained query decoder;
legacy Broken references are retired only under the older version's rules.

Bounds:8MiB total,100 frames,262144 bytes per hash,4096 unique dismissed UUIDs,
safe positive JavaScript group IDs and recursive JSON depth64 on export.
Unknown/duplicate fields, future versions, incorrect bindings and invalid bounds
are refused. Session-namespaced browser history keys are never exported. The
folder picker publishes a synchronized0600 JSON file without clobbering an older
transfer. Neither generic browser storage nor tokens/private filesystem paths
cross the protocol.

Native Settings uses a parented file picker and a generation-checked blocking
worker. Selected files are bounded, regular and opened without following a final
symlink or blocking on a FIFO. Import validates the actual complete native
profile binding and atomically publishes **one** `handover.json` containing its
receipt/source digest and imported native navigation/interface fallbacks. Existing
native preference files take precedence; the first receipt prevents replay.
Newer edits and the receipt survive actual cold worker startup. Missing-transfer
guidance and result notices exist in English/French. Cancel restores visible
focus without a command/effect. Windows support remains deferred.

## Native dispatch observer correction

The controlled12-cohort Intel replay at source `4b745e5` reproduced three traced
keyboard observation-wall p95 failures (107.584522,161.895748,151.607603ms).
Actual production traces, all64 independent clock handshakes and the controlled
single-driver event order conditionally bound callback-dispatch p95 at33.489936,87.300910,50.919916ms respectively. Both
independent evidence reviews accepted this distinction. Later publication and
timer/render work remain observable; these traces do not establish visible
completion or reclassify historical qualification.

macOS method3 measures from before native event posting to the actual search
callback return, or the actual browser-focus acknowledgement used by the
original pointer endpoint. Optional Slint hooks have no product handlers. Only
the qualification fixture installs bounded in-memory timestamps and per-kind
sequences. The helper reuses its existing before/after state observations,
requires exactly one new matching acknowledgement and retains every original
file-observation wall. It adds no per-event arming or wait before timing.

After both input cohorts,64 round trips constrain the offset between the native
and helper monotonic clocks. Every round participates; the metric uses the
conservative upper bound. The actual state containing the acknowledgement is
also a physical upper bound. Missing, ambiguous, incompatible or reordered
evidence refuses qualification. The complete native event log, including all106
search edits, must match the witnesses after positive worker join.

All64 keyboard,64 pointer and20 cold completion samples, real concurrent imports,
bounded24-row pages,1,000 covers and both rendering routes remain required.
Dispatch p95 stays≤100ms; completion p95 stays≤3,000ms. Completion still includes
query expansion, fresh thumbnails and real Tab/End selection. Browser focus
acknowledgement does not claim completion of the pointer handler or rendering.
Linux retains method2's observation-wall upper bound. Producer manifests and
raw admission identify the platform-specific method explicitly.

Regression cases distinguish200ms publication delay from205ms actual dispatch,
retain slow event-posting cost, reject tampered/missing/ambiguous witnesses and
incompatible clock rounds, and replay a provenance-bound64-sample diagnostic
fixture. The latter protects clock arithmetic only. Fresh complete native
qualification and pointer evidence remain necessary; Task17.8 stays open.

## Executable rehearsal and retained evidence

`python3 -I -B scripts/verify-upgrade-handover.py` materializes two private Go
application modules: manifest-verified original source and the current bridge.
Both invoke the actual `NewComposition`/`Application.Start`/`Stop` lifecycle,
storage and durable jobs. They have explicit synthetic paths, disabled network
and no GUI host. This qualifies application services/lifetimes; final packaged
GUI qualification belongs to Task18.

The `upgrade_handover` example executes38 cases: all18 retained schema prefixes
plus current bridge output, each through configured and default legacy paths.
It retains full closed source/backup manifests, profile-backup hashes and all
table/schema snapshots, compares frozen Go upgrade semantics, uses Rust writes,
persists actual preferences/history, restarts, and restores an independent Go
rollback clone while checking that newer candidate/profile bytes remain intact.
All source/candidate file inodes differ. Every immutable archive and additional
legacy payload is checked after use/restart. Timestamp alignment uses only the
original synthetic producer's declared policy. Native settling of its known
pre-move removal intent is a separately asserted custody recovery delta.

The `application_handover` example retains live removal/export declarations,
requires participating second-launch refusal before recovery, launches the
unmodified original Go application for real source-only writes, SIGKILLs and
reaps the Rust owner, and uses explicit native interrupted-owner recovery.
The pending export becomes interrupted; unrelated partial destination bytes stay
unchanged. Original Go activity is checked again after restart, at every observed
relocation stage (including copied files), and after restart at the new root.
Candidate/workspace/profile/journal bytes remain identical across each Go launch.
The separate migration case holds a real SQLite read barrier, interrupts the
owned native generation before its first migration commit, and verifies recovery
to the full18-entry schema with integrity/FK checks. It records that exact
boundary; it does not claim every SQL instruction was individually interrupted.

The rehearsal also composes the existing cumulative failure suites: verified
online snapshots and migration restoration, all18 exact prefix/query oracles,
removal/export publication compensation, journal alias/foreign-file preservation,
and native relocation forced termination at8 durable stages with real volume,
permissions and disk-full faults. Those existing source-level and native evidence
gates retain their original assertions.

`target/quality/upgrade-handover.json` contains full observations, current/frozen
source hashes, driver/helper identities, compiler-reported native executable
identities and actual toolchain. Rust quality runs the new rehearsal on ARM
macOS, Intel macOS and Linux x86-64 and retains it with cumulative/native evidence.
Native foundations additionally executes the transfer callback/cancel/focus/
repeat-import/cold-start path in both languages at both observed scales.

The [PR68 qualification record](https://github.com/01max/librairii/pull/68)
indexes provider run/artifact identities, archive digests, qualified source/merge
trees, cumulative counts and both independent source/evidence reviews. A passing
workflow alone does not admit the evidence: review the raw38-case snapshots,
four retained executables and their source/compiler bindings together with the
complete native quality, scale and Linux speech receipts. Task17.8 remains the
admission gate until that review is complete. Local reports live under
`target/quality`; independent audit receipts under `target/task17-diagnostics`.
Do not infer completed Task18 package, signing or installed-host qualification
from these rehearsals.
