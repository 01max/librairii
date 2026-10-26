# Native presentation foundations

Integration base: `08cdae9fc9bd690d2927731252f40b5bc48953e5`. Section branch:
`rust/rewrite-14-native-foundations`; every subtask has its own commit.

The production composition promotes the selected Slint 1.18.1, Winit 0.30.13,
Skia 0.153.3 graph from section 2. It uses the approved royalty-free license
route, with the actual AboutSlint widget accessible through the application
menu. The qualified AccessKit 0.19.1 AT-SPI Enabled/Sensitive backport is copied
byte for byte to `third_party/accesskit-atspi-common`; its upstream, backport,
license and Chromium notices remain attached. `.config/native-toolkit.json`
seals that source tree. This does not claim product package qualification.

The process main thread owns the native event loop, window, menu and view
projection. One joined controller thread owns startup inspection and storage.
There are at most 16 pending presentation commands and one coalesced snapshot.
Shutdown has a separate atomic path so a full command queue cannot prevent
closing. The event loop remains alive until the worker finishes; storage owners
and their journal/worker cleanup complete before process exit. Constructor or
renderer failure also drops and joins the controller. No browser, HTTP bridge,
HTML renderer, detached storage task or production network call is introduced.

Embedded catalogs retain the product message identities and synthetic assets
are compiled into the native UI. Developer overrides bypass normal profiles;
existing unconfirmed overrides remain refused before window construction. The
historically admitted skeleton refusal identity remains as a regression of
that boundary, rather than a claim that the new native runtime is absent.

Section 14 supplies shared presentation behavior; complete domain workflow
views and final installed-package acceptance remain sections 15 and 18.

## Native layout adaptations

The OS owns title bars and window buttons on macOS/Linux; no simulated macOS
chrome is painted. Slint's system palette and standard controls supply native
contrast, font fallback and widget metrics. The approved labeled action rail,
separate Collections/Shelves navigation, catalog panel, central stories and
right inspector retain their hierarchy. Wide starts at 1180 logical pixels;
850–1179 retains navigation and opens details separately; below 850 navigation
is a dismissible overlay. The supported minimum is 560×400 logical pixels.
Navigation, results and inspector scroll independently without horizontal
scrolling. A toolbar disclosure remains available when navigation collapses;
Escape/backdrop restore its focus. Layout thresholds use observed window size,
not self-dependent layout constraints. Selection and query belong to the Rust
presentation owner, independently of panel visibility. Full workflow-specific
controls remain section 15.

## Bounded browsing and selection

The native list and both cover densities use the toolkit's virtualized ListView.
The model is grouped into responsive cover rows; list uses one story per row.
Only the current SQL page is projected (existing default 24; request ceiling
100, Shelf participant ceiling 25). No all-library artwork/model allocation is
introduced. Counts and page commands retain full result totals. Artwork is read
by opaque story/artwork IDs and decoded through the existing per-library bounded
thumbnail cache on the controller worker; the UI receives at most one bounded
page of RGBA previews. Images use contain fitting and captions remain outside.
Unavailable approved UUID members retain their cached title and explicit warning.
Mode changes keep selection and criteria independent and persist to the existing
library-bound native navigation file. The real-library controller test exercises
all modes, independently reads the durable preference and reopens after shutdown.

## Accessible shared controls

Application controls and native application/Edit menus provide English/French
names. Native edit accelerators operate on the focused text widget and retain
its selection and undo stack. Menu changes are coalesced; handlers execute on
the main thread without an unbounded UI event queue. Custom story rows expose
list/item counts, selection, enabled state, actionable selection and a visible
focus border, including virtualized offscreen keyboard seeks. Broken and
unavailable warnings carry explicit text independent of color. Shared tag chips
include their definition/value text; decoration does not supply their identity.

The pinned toolkit has Boolean checkbox accessibility, so the shared mixed
assignment control uses an explicitly labeled Mixed/Set/Clear choice with a
truthful current value instead of presenting Mixed as unchecked. Progress has
a named semantic range/value and static indeterminate presentation. Application
panels, covers and progress introduce no animated movement; the OS/toolkit owns
standard transient control feedback. These are permitted accessibility/control
adaptations of the approved hierarchy, checked by native fixtures below.

## Modal and pending-operation behavior

Dialogs overlay the existing layout. They record the invoking focus target,
choose initial focus, contain Tab/Shift-Tab, dismiss on Escape/backdrop, and
restore the invoker. Opening settings from a compact sidebar suspends that
sidebar; dismissing settings returns to its settings button, and a subsequent
Escape closes navigation and returns to the disclosure. Background controls
and native edit actions are disabled while a dialog owns focus.

Close during pending work offers Keep working and Stop and close; the latter
requests the owned runtime's safe shutdown rather than terminating a worker.
It is disabled while the caller reports a non-cancellable commit. Backdrop or
Escape means return to work and never confirms a mutation. Settings language
commands persist before updating labels and retain the active metadata locale.
Setup/recovery entry buttons stay available; complete folder/adoption/recovery
transactions and workflow-specific dialogs remain section 15.

## Semantic navigation and response acceptance

Back/Forward, Library, Collection and Shelf commands use core typed navigation;
Collection and curated Shelf IDs remain distinct even with the same numeric ID.
The existing versioned library-bound `navigation.json` retains frames, criteria,
sort/page, selection and the independent mode preference, with 100-frame and
8 MiB limits. Every accepted transition persists before projection. Opening a Collection reads
its saved membership into the navigation frame; later search edits use that
frame's actual criteria while retaining the named context, matching the preserved
frontend. Back/Forward restore those criteria without silently replacing them
with the saved definition. Empty saved selection falls back to the first visible
story, matching the preserved frontend's inspector behavior. There is no
URL/address bar, browser history or frontend event bridge. Official metadata
uses the frozen provider's `en-GB` default independently of interface language.

The 16-entry ordered intake coalesces only adjacent, unstarted search requests.
History, selection and language commands retain order and report backpressure;
shutdown closes intake through its independent path. Responses carry the admitted
request revision and actual library activation token. A superseded query cannot
replace the current view, and a token from a different activation is rejected
even when an independent clone preserves the same library UUID. Selection
commands carry the originating query stamp and stale selections are ignored.
The one latest snapshot remains bounded. Native tests exercise persisted history,
mode independence, overlapping group IDs, a burst of searches, shutdown/reopen,
queue ordering/backpressure and obsolete activation/query rejection.

## Source and notice promotion

The production/tooling dependency inventory retains the exact vendored AccessKit
source checksum and its original license/Chromium notices. The native Skia notice
supplement is verified against its pinned source ref and payload hashes before
copying. Published `LICENSES/` directories are retained recursively. Dependencies
whose registry archives omit license texts have `.config/upstream-notices/`
supplements fetched from immutable official upstream revisions, with URLs and
SHA-256 provenance. Most match the registry VCS revision; drm-fourcc explicitly
records a later original upstream notice. dispatch publishes only an MIT manifest
declaration: retain that declaration/author verbatim plus the attributed standard
SPDX MIT text without inventing copyright details. Its installed redistribution
notice review remains section 18. The selected Slint royalty-free route is explicit; its
alternative upstream texts do not assign a license to Librairii.

Exact duplicate/license/maintenance decisions in the dependency registry expire
2027-01-24. Informational maintenance notices retain the qualified graph; no
vulnerability advisory is suppressed. Bincode is locked optional support of
typed-index-collections and is not selected by the active graph. The yanked
Windows-only clipboard-win 5.4.1 entry is replaced by compatible 5.4.2; Windows
remains deferred. Native Slint/Winit/Skia versions/features are unchanged.

## Fixture contract

`docs/rewrite/native-reference.json` records the original approval, concept names,
renderer identity and editable/export digests. Byte-identical original reference
files are retained under `docs/rewrite/ui-reference/` and verified before capture. The approved HTML remains design
evidence; it is never imported by the native executable. Native evidence compares
the saved source/manifest hierarchy with directly rendered captures. The browser's
local-file URL policy prevented opening the HTML preview during this task; no
alternate browser route or production HTML engine was introduced.

`verify-native-foundations.py` builds the actual desktop projection/example on a
matching host, displays both concepts in English/French at wide/intermediate/
compact sizes, exercises all three story modes and observes scale factors 1/2.
It requires clean committed source and retains 24 concept/mode/density/language
frames per scale plus opening, setup, recovery, actual Slint attribution,
keyboard-seek and static-progress frames. Model/keyboard assertions cover text
palette contrast >=4.5, zero implicit review decisions, literal UTF-8 input, modal
Tab containment/Escape/restoration, nested sidebar dismissal, offscreen selection,
pending commit close safety, empty first-run/recovery projections and static
progress. Intrinsic portrait/square/landscape fitting and external captions are
checked against the source/layout and actual captures; they are not pixel-diff
assertions against HTML. The Linux private-bus fixture additionally
inspects actual AT-SPI roles/names/enabled/sensitive/selection/focus state and uses
X11 keyboard input. This is native component evidence, not physical HID, VoiceOver,
full domain workflow transactions, installed packages or the section-16 budgets.
The required matrix executes it on Apple silicon, Intel macOS and Linux x86-64;
Task 2's existing Wayland qualification remains separate from these X11 fixtures.

Startup inspection now reports an opening view before expensive read-only work,
with cancellable inspection and override-first isolation. Invalid native UI state
is preserved before library activation. Active preference failures fence queries
and hold the actual profile/library session until the user closes; then joined
runtime cleanup closes resources before releasing ownership. Valid Shelf warning
preferences are recognized alongside settings/navigation. First-run language
selection creates independent preference containers without creating a library.
These real isolated-profile cases are admitted in the cumulative test baseline.
