# Collections and curated Shelves

Collections are dynamic saved queries. Shelves retain explicitly approved story
UUIDs, with optional Collections supplying human-reviewed update proposals.
Existing saved Shelves migrate to Collections with their identity and non-Broken
criteria preserved. Collections combine as a deduplicated OR union.

Shelf membership changes only through explicit creation, manual curation, or
review approval. Manual additions and explicitly kept stories are protected.
Exclusions persist until reversed; deferrals store no decision. Missing library
stories retain their membership and resolve again when the same UUID returns.
Historical acceptance origins remain distinct from current source matches.

Reviews are snapshots. Apply revalidates source definitions/results, availability,
and Shelf decisions within a transaction. Invalid sources permit healthy-source
additions but suspend inferred removals. Source detachment and Collection deletion
require an explicit impact review; retained orphaned members become protected.

The dedicated Broken flag, its independent storage, legacy tag migration, warning
presentation, atomic mutation, and exact Undo remain from the completed Broken
feature. This change supersedes its visibility and export policies: flagged
stories remain visible and counted, while every export skips them with a reason.
Supported legacy queries lose built-in Broken restrictions recursively without
losing unrelated predicates or ordinary user-tag semantics.

Exports use current Collection results or approved Shelf UUIDs, without pending
proposals. Prepared eligibility is revalidated, and queued copies check current
flags before copying. Historical shelf/shelves exports retain their original
dynamic-query meaning; new curated exports have distinct source discriminators.
No filesystem synchronization or automatic membership updates are introduced.

The implementation follows the `introduce-collections-and-curated-shelves`
OpenSpec change. OpenSpec is maintained in its separate nested repository; this
document records the application repository's behavioral contract.
