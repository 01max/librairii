# Task 13: setup, language, relocation and recovery

Task 13 implements the native service boundary for setup/settings and complete
library relocation. Its dedicated branch is `rust/rewrite-13-setup-relocation`;
[PR64](https://github.com/01max/librairii/pull/64) targets
`rewrite-in-rust-native-desktop`, based on
`44ea386f7b84ae2861aa77aaea9956dddeb7c6fa` after PR63 was merged.
The working Go/frontend application and frozen Go oracle remain unchanged.
Native views, folder-picker presentation, menus, keyboard focus and packaged
acceptance are Tasks 14–15 and later gates. This service slice does not make the
Rust desktop executable a replacement application or authorize any merge.

## Subtask delivery

Every code-changing subtask has a distinct Conventional Commit and an
`OpenSpec-Task` trailer. Review corrections are separate commits.

| Task | Initial dedicated delivery | Evidence |
| --- | --- | --- |
| 13.1 Explicit setup/adoption | `3d9fe36` | Exact empty/missing selected roots, read-only legacy discovery, confirmed independent adoption, invalid/missing-library recovery; `0f25cb5` adds cold-start preparation and usable native suggestion |
| 13.2 English/French | `339da88` | Frozen catalog identities, system-first choice, ordered historical fallback, formatting; `66e43dc` and `cd8f36d` preserve existing behavior/message keys |
| 13.3 Serialized language | `254dfdd` | Persist before presentation, merge newest language with root changes, buffered Settings command, retained unrelated state |
| 13.4 Target validation | `39ef98f` | Retained path/parent identity, aliases, overlap, writable/durability/capacity checks; `af0ffd9` handles current-root no-op |
| 13.5 Exclusive lifecycle | `52cfddf` | Actual closed native ownership and full manifest; `ff21b67` joins/rebuilds services; `7ae6897` makes mutation/move admission have one winner |
| 13.6 Verified relocation | `d04bc76` | Destination-owned streaming copy, full verification, actual DB readiness, no-clobber publication and independent profile commit |
| 13.7 Restart/cleanup | `e3569f7` | Immutable independent journal, retry/cancel, uncertainty fencing and acknowledged idempotent source retirement |
| 13.8 Missing-library recovery | `4140082` | Same-UUID locate, no silent replacement, override isolation; `5402145`, `ecef4a3`, `09068d8` strengthen final revalidation and explicit-root recovery |
| 13.9 Independent native qualification | `c673172` | Real SIGKILL, permissions, full-disk and separate-filesystem rehearsal; `7f4f688` binds execution to Cargo's actual artifact |

## Setup and independent adoption

Startup inspection performs no creation or repair. It distinguishes a new
installation, an existing native profile, an identifiable closed legacy library,
and explicit recovery. Invalid settings, an incompatible/nonempty legacy default,
a missing configured root/database, or ambiguous recovery evidence never become a
new empty collection. The new native suggestion is the native data container's
`library` child; the Go default is used only for discovery and exclusion.

After an explicit setup or language choice, `prepare_native_profile` can create
missing native configuration/data containers through retained directory owners.
It validates namespace names, existing settings and legacy separation before
effects. It creates no library identity or database. Picker dismissal/startup
inspection do not call it. Existing invalid bytes are preserved. Confirmation
initializes the exact selected root with its UUID, six managed areas, unchanged
schema 001–018 and a version-2 native record; profile authority publishes last.
A failed attempt retains its real owners for retry rather than treating a parsed
path or status as permission to overwrite.

In-place Go adoption remains refused. Explicit adoption verifies a complete
closed source and creates a physically independent consistent clone outside Go
profile/default/source discovery. Legacy settings and source bytes remain
source-directed through native reopen and relocation. Earlier version-1 native
records retain their stricter data-container scope; version 2 records explicit
selected-root authority and protected legacy roots.

## Language and Settings projection

English and French are the only explicit choices. Saved language takes priority;
first-install selection uses the first OS preference, choosing French for a
French preference and English otherwise. macOS reads the bounded system language
preference with a deadline and joined process/reader; Linux uses its established
language environment precedence. Override mode bypasses production profile, home,
default-root and system-language reads and uses in-memory language only.

The Rust projection uses the existing English/French catalogs and message IDs,
scan-once placeholders, plurals, number/date formatting and original historical
outcome detail. Ordered catalog reversal preserves frozen collision behavior;
unknown English historical summaries retain their original text. Language
changes persist through the serialized native profile before the presentation
and menu revision update. They do not own/reset query, selection, editor input,
metadata content locale or operation state. Settings commands arriving before UI
readiness are coalesced and delivered once. Actual native menu/view rendering
and accessible keyboard interaction remain presentation work.

## Target and lifecycle boundary

Inspection is read-only and retains the actual nearest existing ancestor and
empty selected directory, if present. It resolves aliases, rejects final links,
special files, unrelated destination content, source/configuration/Go discovery
overlap and determinable insufficient space. Physical ancestry uses filesystem
identity, including case/decomposition aliases. Actual create-new/flush/owned
probe retirement checks writability. A current-root alias produces `Unchanged`
without draining the live DB or changing preferences. Execution revalidates the
retained observations; stale preparations never authorize replacement.

The lifecycle gate immediately refuses active mutation-capable commands without
changing generation. Once relocation wins, new mutators refuse before callbacks
or effects. With intake closed, it checks the actual job coordinator and
persisted queued/running operations before and after draining retained
reader/artwork uses. The desktop
session joins workers, invalidates export preparation and thumbnail state,
flushes/drops logging and closes the writer/readers before consuming the native
owner into `ClosedNativeLibrary`. The manifest enumerates every managed area and
permitted additional root entry using descriptor-relative identities, bounded
accounting, file sizes/hashes and change witnesses. No caller-supplied idle flag,
profile document or serialized manifest grants filesystem cleanup authority.

## Copy, commitment and restart

Relocation uses an exclusive workspace on the destination filesystem. An empty
selected volume root is itself retained as the workspace; it is never renamed
or unmounted by product code. Other targets use a private sibling workspace and
no-clobber publication to the exact selected folder. Files stream in 64 KiB
chunks into create-new destinations with source identity/size/hash checks,
read-back verification and file/directory durability. Existing retry files must
match the complete recorded bytes; partial/changed files are preserved/refused.

Before commitment, the service verifies both complete manifests, identity and
schema/integrity, actually opens/closes destination writer/read workers with
mutations gated, and revalidates the source, destination, record and lease again
at the final boundary. Atomic independent profile publication is the sole commit
point and merges the latest language. A publication error that cannot prove the
old authority remains is explicit `Uncertain`; both real owners stay retained
and ordinary use is fenced. Cancellation applies only before commitment.

The immutable, bounded native journal lives outside both library roots. Restart
qualifies the actual profile-selected root, native namespace, record, UUID,
lease, filesystem identity and DB before workers or cleanup. The configured root
selects source before commitment and destination afterward; a missing committed
destination never falls back to the stale source. Explicit precommit Retry can
finish a verified retained workspace; Cancel/resume preserves candidate bytes,
retires only the qualified journal and permits a later move.

Source cleanup checks each recorded filesystem identity and hash, retaining new
or changed bytes. A durable append-only retirement receipt precedes control/root
removal, making the final missing-control/root gaps restart-idempotent. Missing
source without that receipt remains pending, since a drive may be disconnected.
Pending cleanup can explicitly be retained without blocking every future move.
A live precommit failure resumes the real source and reconstructs its services;
postcommit/uncertain failure stays in recovery. Every successful activation
rebuilds queries/files/imports/jobs/removal/export/artwork/logging/diagnostics as
one new storage generation. Old preparation IDs and callbacks are invalid.

## Explicit recovery

Retry revalidates the saved native library. Locate accepts only the saved UUID,
compatible DB, qualifying closed native record/profile relationship and safe
scope; foreign journals/reserved control leftovers refuse before SQLite writes.
With a pending move, role validation prevents selecting the stale source as the
missing committed destination. Explicit locate preserves other retained copies
and retires only the qualified pending move journal. Language is retained.
An unused native data container may be absent when the actual selected library
is elsewhere: its logical separation remains checked without recreating it.
Override mode keeps relocation disabled with the existing explanatory message.

## Verification and review

The local final implementation passes `cargo xtask check` on macOS ARM:
581 tests, zero skips, strict Clippy, formatting, cumulative discovery,
preservation/provenance, dependency/license/advisory policy and compile-fail
documentation tests. `make check` passes for the retained Go/frontend app,
including 207 component tests plus release, responsive, visual, accessibility
and performance checks. New Rust identities are additive; historical baseline
order, frozen sources and checker logic remain unchanged.

The native rehearsal builds through Cargo JSON artifact selection, records and
rechecks executable/source digests, and owns its disposable 128 MiB image. macOS
uses APFS; Linux uses an ext4 loop image. It requires different source/destination
devices and actual exact selected mount-root relocation/reopen. Eight actual
SIGKILL boundaries cover workspace, journal, completed-file copy, publication,
precommit, committed profile, acknowledged retirement and final source removal.
Restart verifies profile authority, French, original payload hash and actual DB
reopen; precommit journal cases exercise actual retry. Independent real EACCES
and ENOSPC faults preserve source bytes/authority and reopen the source.
Receipt publication requires positive disposal of the owned image. macOS busy
detach is bounded and can force-dispose only that private image. No production
root or ambient external drive is formatted or mounted.

Immutable three-platform checkpoint receipts and source reviews are recorded in
`section-13-native-evidence.json` and `section-13-review.md`. The checkpoint is
691f08d; the subsequent mutation-admission correction is 7ae6897 and has a real
failing-before/passing-after regression plus the full local 581-test gate. Final
pushed-head workflow/source/artifact receipts are recorded on PR64 and in the
local canonical handover; checkpoint receipts are never relabelled as later-head
execution. All three native
targets must pass their complete gate, frozen Go isolation rehearsal and the new
native relocation rehearsal; earlier partial checkpoints cannot substitute.
This evidence covers the service slice on native CI hosts, not every removable
filesystem, physical-device disconnect, GUI, package, release or global cutover.
