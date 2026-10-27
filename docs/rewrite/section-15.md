# Complete native workflow surfaces

Integration base: `f62bab08e9f7db70c5b7b6cce8dbf69cbd121921`, after PR65 and its
query paging/history corrections. Branch: `codex/rewrite-15-native-workflows`;
the task PR targets `rewrite-in-rust-native-desktop`. Task15 is in progress.
The frozen Go source, approved visual references and qualified toolkit graph
remain the contract. This section does not authorize runtime retirement.

## Search, filters and sort — 15.1

P14–P18 retain their existing query engine and canonical navigation. An owned
main-thread draft composes rapid edits before their coalesced worker response,
keeping literal spaces, case and accents in the editor. The collapsed Filters
entry opens searchable native language/import-status/tag criteria, Boolean
Any/Tagged/Not tagged, additive choice values and explicit sort. Only facet
input goes through the frozen codec; names, nested unions and tag criteria do
not receive an extra saved-query normalization pass during editing.

The active summary remains visible while filters are collapsed. The dialog
lists removable criteria, including unavailable saved references and combined
queries. Clear one/all resets the page while retaining explicit sort; clearing
tags retains other groups. Clear all returns to search; individual removal
returns to the filter search field. Closing preserves criteria. Each tag definition, language and import-status group has a keyboard-operable collapse button that retains selected values; searching reveals matching options. Empty query sorts display the frozen name default. Shelf sorting explicitly switches between manual and view order in the same navigation frame, including Back/Forward and persisted restoration. Language choices come from a storage-owned facet API. Storage query
failures remain errors with history recovery, never unfiltered success.

The form and its lists scroll independently. Native Tab focus reveals lower
controls using actual coordinates, and Shift-Tab/Tab wrap between the first
field and Close. Inactive modal footer controls reserve zero height. Native
editing targets the focused modal field; background editing stays disabled.
Dropdown bindings retain actual selection when models/language change.
English/French controls translate; user-authored tag labels remain verbatim.

Evidence: two additive pure query-editor regressions, the existing desktop/UI
suite, real production callbacks backed by a fresh 60-story SQLite library,
and shown compact English/French top/action captures. The live controller stays
owned across capture frames. Assertions cover OR-within-choice/AND-across-group
results, exact sort persistence, literal trailing space, collapse, clear/focus,
the actual displayed dropdown index and complete focused-action bounds. macOS
AppKit Select All/Undo/Redo also exercises the modal editor and checks that the
background search remains unchanged. The 24-frame foundation matrices and all
previous callback/menu checks remain required. The observed Rust baseline adds
two identities to its unchanged 608-entry prefix; historical source identities
are preserved. Final current-source three-host and screen-reader acceptance is
the 15.13 gate; these local checks do not claim that later work has passed.

## Story details and removal — 15.2

The storage-owned single-story observation returns display provenance and complete
archive facts from one SQLite snapshot, retaining original native filename bytes.
The controller converts descriptions with pinned html5ever0.40.1's HTML5 tokenizer
([upstream interface](https://docs.rs/html5ever/0.40.1/html5ever/tokenizer/index.html)).
Only inert text crosses the UI boundary: paragraphs, lists, plain less-than signs
and all named/numeric entities survive; script/style/iframe/object/template/noscript
content is omitted. No DOM, link, image request or script execution is created.
The tokenizer's phf0.14 atom tables coexist through three exact duplicate-version
exceptions with the unchanged qualified AccessKit phf0.13 graph. The complete
license and bans audit passes; historical toolkit pins are unchanged.

The inspector shows tags, independent per-field sources, official metadata locale,
publisher/author, duration/ages and freshness. A separate technical disclosure
contains UUID, original filename, verification and full SHA-256. Story changes and
reopening reset inner scrolling and disclosure state. Compact details keep the
story title above bounded artwork; lower content remains keyboard-scrollable.

Removal is a separate confirmation with a frozen library generation, query stamp,
story ID and UUID. Keep is initially focused; Escape/Keep returns to details and
the Remove invoker. Admission immediately disables dismissal while the controller
uses the existing durable trash/intent/rollback service. Required rollback keeps
its actual owners and reports recovery; ordinary intake stays fenced after a
recovery event. Confirmed outcomes survive refresh failures. Originals remain
untouched, and no trash-restore capability is introduced.

Evidence: the cumulative 31-test desktop/UI run passes with zero skipped tests;
two new observed description identities append to the unchanged baseline prefix
(612 total). Actual production callbacks import a preserved archive into a fresh
owned library, verify text/facts/provenance, Keep focus, stale-target refusal,
pending-dismissal refusal, committed refresh, original-byte preservation and
retained managed trash bytes. Shown English/French compact captures exercise the
technical button with native Space and confirmation Escape; reopening asserts a
zero inner scroll offset. The wrapper requires these callbacks in every native
receipt. Full section/cross-host/accessibility acceptance remains at15.13.

## Remaining workflow work

15.3–15.12 compose tags, Broken/Undo, Collection and Shelf
curation/review, imports/sync/exports, durable History/notices and complete native
setup/settings/recovery/diagnostics. 15.13 independently reviews the complete
native surfaces and refreshes visual/accessibility evidence. Resource budgets,
handover, installed packages and cutover retain their later task boundaries.
