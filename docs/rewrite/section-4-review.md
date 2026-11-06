# Task 4 independent review

Review basis: integration `244f4494e2bf1fcc5cc31c8caba61171ca5f8771` through
implementation/evidence head `15be09a700fb1b7d70aeb0113332eeff57b3157a`.
Two independent read-only reviewers used the code-review skill's Standards and
Spec axes. Neither ran Cargo or edited source. Target/provider execution is
separate evidence in `section-4.md`.

## Standards

No required findings remain. The initial capture's working-checkout execution
and overstated paging description were corrected in `98acec9`. Go now builds
from verified manifest-only source buffers, excluding extra checkout sources
and inherited Go overrides. The fixture accurately claims out-of-range paging
and sort metadata.

The final reviewer found no documented-standard violations or additional code
smells in the diagnostics, private snapshots, retained copy ownership, native
name races or storage rehearsal. The follow-up through `15be09a` verified exactly
three additive discovery identities at their matching baseline positions, with
all prior identity order and historical source/index/codec/derivative/baseline
fields unchanged. Only the current map digest changed; checker logic stayed
byte-identical and no policy was weakened.

## Spec

No required findings remain in the supported Section 4 storage routes. The
initial review identified missing affected-item reporting, historical path
inventory, real target-name refusal evidence and Go/Rust lock interoperability.
Commits `115c9d7` and `e98611e` address those findings.

The final reviewer accepted the explicitly supported fallback: unconditionally
refuse in-place adoption, use a verified independent clone/native namespace,
retain owners through resource closure, and preserve/refuse unverified recovery
or retargeting. The shared-lock/source-isolation rehearsal supports this storage
boundary; complete Go application bridge/preferences and successful relocation
remain Sections 17/13. Windows remains deferred.

The source-mutation evidence was strengthened: both Go calls append to the title
and the source DB bytes must change, while exact candidate/profile bytes stay
unchanged. A newly persisted pre-move intent establishes the overlap; only that
probe-owned intent is settled before actual close. This runs unchanged frozen
Go storage services through a synthetic driver, not the full legacy executable.

The Spec reviewer required passing current native/provider results before
checking off 4.11; a source review alone is not execution evidence.

Required remaining findings: Standards **0**; Spec **0**. Native/provider
acceptance remains independently recorded, with no release/cutover claim.
