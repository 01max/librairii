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

## Remaining workflow work

15.2–15.12 compose details/removal, tags, Broken/Undo, Collection and Shelf
curation/review, imports/sync/exports, durable History/notices and complete native
setup/settings/recovery/diagnostics. 15.13 independently reviews the complete
native surfaces and refreshes visual/accessibility evidence. Resource budgets,
handover, installed packages and cutover retain their later task boundaries.
