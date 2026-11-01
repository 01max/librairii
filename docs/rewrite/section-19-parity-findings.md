# Task 19.1 semantic audit findings

The audit of all sixty frozen parity rows found three missing acceptance
witnesses. The earlier 197 links/172 distinct passing tests and native callback
reports established execution, but did not cover these subcases. The original
observations and review findings remain retained; no runtime defect or failure
cause is inferred from a missing test.

| Parity rows | Missing witness | Dedicated regression |
| --- | --- | --- |
| P29 | Arrow reordering beyond 100 approved members, including rank gaps and a changed order surviving reopen | [`shelf_order.rs`](../../crates/librairii-desktop/tests/shelf_order.rs): `controller_reorders_beyond_one_hundred_gapped_members_across_pages_and_reopen` uses the real controller and database with 145 members. It checks both directions at indices 100/120, the page-6-to-5 boundary, full UUID order, selected identity/focus, refusal under a temporary refinement and a final non-original order after native reopen. |
| P30/P49 | Actual removal followed by manual copy-out/reimport without resurrecting deleted manual tags | [`removal_reimport.rs`](../../crates/librairii-desktop/tests/removal_reimport.rs): `actual_removed_archive_reimport_reconnects_shelf_state_without_manual_tags_after_reopen` uses the actual import executor, removal custody and retained trash bytes. It copies the archive outside the library, reimports through the executor, forces a new numeric ID and verifies absent old manual assignments with exact Shelf UUID order, origins, retention and exclusions across native reopen. |
| P33 | Applying a healthy addition while another linked source remains invalid | [`shelf_incomplete.rs`](../../crates/librairii-storage/tests/shelf_incomplete.rs): `healthy_addition_commits_while_invalid_source_keeps_all_removals_suspended_after_reopen` independently evaluates the healthy source's exact UUID set, then commits Add before repairing the other source. It verifies the new healthy origin, retained unmatched/unavailable approved members and continued incomplete/not-Up-to-date status after database reopen. |

Review strengthened the draft tests before execution: the P33 unmatched title
must not contain the literal search substring; the P29 final order must differ
from the initial order to make persistence meaningful; tagging commands must use
the actual storage adapter types. The offline reader's initial separate-read
input-binding defect was also corrected with a counterexample regression.

The [audit contract](section-19-parity-audit.json) now contains 201 links to 175
distinct cases. The three new tests have separate required discovery identities
for both Macs and Linux, added after the complete original baseline. No original
test, fixture, oracle, assertion or numerical ceiling was removed or weakened.
The tests exercise controllers/storage/files without constructing a window;
actual installed native GUI, speech, performance and final package review remain
their own evidence scopes.

Source review alone does not close these findings. The exact three new identities
must compile and pass on all three target hosts, with matching source/discovery/
JUnit and zero skips or failures. The resulting cumulative suite should contain
632 tests on each Mac and 634 on Linux, including every previous applicable case.
An older source's 629/629/631 results cannot satisfy the new contract. Retain the
current raw archives, failed attempts, offline audit output and both independent
review receipts with their original source/target/package hashes.

19.1 can close after these execution witnesses and the completed independent
row review are reconciled. The final signed-byte checks, final manifest and
independent release rehearsal under 18.2/18.3/18.10/18.11/19.6/19.7 remain open.
Human-controlled signing, approval, publication and cutover are unchanged.
