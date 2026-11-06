# Trash and diagnostics during the native rewrite

Removal takes the managed library copy out of the catalog and keeps its current
archive bytes in the library's `trash/removals` directory. The original import
source stays where it was. Removal also works for Broken or corrupted copies.
Cancelling an unfinished removal restores the managed copy; startup does the same
for interrupted removals whose catalog deletion did not commit. A completed
removal remains in trash. Shelf membership and exclusion decisions retain their
story UUID references, including while the story is unavailable.

There is no in-app trash restore or empty-trash action. To recover a story
manually, first copy its archive from trash to a separate folder, then import
that copy using the normal import action once available. Keep the trash copy
until the import has succeeded. Inspection can refuse a corrupted archive;
removal does not repair it. Reimporting an archive with the same UUID reconnects
retained Shelf decisions. The deleted story's tag assignments were removed with
its catalog row and are not restored by reimport; add those tags again as needed.

Diagnostics export runs only after you choose a destination folder outside the
library. It creates a new ZIP without replacing an existing file. The report
contains an aggregate/version/platform manifest and sanitized local event logs,
limited to five files of at most 256 KiB each. It contains no library database,
archive contents, story/group names or identifiers, tokens, or personal paths.
The application does not upload the report. You decide whether to share it.

If export is cancelled before publication, its owned incomplete temporary is
removed. If publication cannot be confirmed, the application reports uncertainty
and preserves the actual output. Keep the selected files and reopen the library
through recovery; a completed file is never removed as compensation. English and
French diagnostic messages use stable reason identities without including raw
filesystem paths or storage error text.

These services currently have headless tests. Their native pickers, progress,
localized dialogs and shutdown wiring remain part of the later desktop UI tasks.
