# Frozen query reference inputs

These files are byte-identical copies of the already completed synthetic Go
capture, not regenerated outcomes. Oracle: `73a93303a9a44cf0a92eaf525331561ed8c04460`.
Capture source: `section-07-query-output-capture/attempt-02-color-v1` in the
integration handover artifacts. The preceding rejected fixture-color attempt
remains historical; it is not used as expected query output.

| File | SHA-256 |
| --- | --- |
| candidate.json | c17f99dc76a72277f7c1a66391476ef185e28b7a0bc875f128f8f73f8fe2f342 |
| seed-context-A.json | 8bec91accfde93a44318cfa4495424a3affff53e7827d4aaefe19a4cb69c537c |
| seed-context-B.json | 669f2dcd4afd211da8c0f36d8d9f62e71f3a2b3b201633abed8851e12e08ffdf |

The Rust test initializes the real unchanged 001–018 migrations in isolated
roots, then restores supplied rows with bound values and ordinary constraints.
Metadata is inserted while snapshots are staged, then activated in original
order. Fixture SQL strings are documentation and are never executed.

All 24 requests run through Rust Count, Search and full-scope visitation. Every
successful field/order/raw UUID is compared; q24 must fail all three APIs rather
than becoming an empty match. No timestamp, path, identity or output fields are
normalized. Capture paths are historical synthetic roots, not runtime inputs.
