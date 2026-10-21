# Task 6: jobs, recovery, history and shutdown

Base: `7883a73d3a991ad50d174f2c456365aad029d039` (Task 11 merged).
One dedicated PR targets `rewrite-in-rust-native-desktop`, with a separate
Conventional Commit for each implemented subtask. Linux x86-64 and macOS
ARM/Intel are the supported admission targets; Windows remains deferred.

## 6.1 — Durable operation state and compatibility

Creation and item-start transitions use the admitted Database writer. Item
completion and its parent counter update share one guarded transaction. Finish
requires every item to be terminal. Interruption preserves terminal items and
recorded bytes, cancels unfinished items and saves its reason atomically.
Acknowledgement readback uses the reserved same-writer barrier and compares the
entire immutable command scope and payload. Refusal, rollback, acknowledged
commit and unknown outcome remain distinct; a later refusal cannot erase an
earlier unknown outcome. Public snapshot values convey observations, not file
custody or activation authority.

This ports the retained borrowed terminal interface from original reference
`8d91ba2f` and corrected signed-ID SQL implementation from `c2eb8ec6` onto the
current integration base. Those references are unchanged; no proof-union branch
was merged. Creation, exact Go JSON scope escaping, start transitions and legacy
presentation envelopes are additional production repository methods. Migrations
001–018 stay unchanged.

Raw legacy operation IDs, signed persisted item IDs, nullable outcomes and
immutable resolved story identities survive reads and reopen. Historical
`shelf`/`shelves` sources retain Collection interpretation and both presentation
aliases; curated Shelves remain separate. Absolute destination and managed-file
internals are omitted from presentation. Input validation uses the frozen
whitespace policy and a 16 MiB retained-command budget, not an export item cap.

Validation includes the preserved terminal port and 14 SQLite tests plus real
creation/start/completion/readback/reopen, legacy dynamic/curated source JSON,
invalid creation rollback and contradictory header readback tests. The private
controlled jobs engine remains historical model evidence, not production effect
composition.
