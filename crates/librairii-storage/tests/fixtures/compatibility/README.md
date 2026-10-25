# Migration-prefix query oracle

`prefix-queries.json` records actual Go Count and Search returns for six requests
on each of the eighteen frozen populated migration prefixes: 216 API calls.
It supplements the admitted full schema/row/Collection snapshots without
changing or regenerating anything in `preservation/corpus/`.

The requests cover default paging, accent-insensitive search, literal wildcard
characters, a deep page/import ordering, a live boolean reference (including its
absence on early prefixes), and nested OR. Full page JSON, including Broken,
source attribution, compatibility and timestamps, is compared without scrubbing.
Only the existing producer's `fixTimes` policy sets synthetic timestamp columns;
NULL timestamps and all other fields remain unchanged.

Capture from the repository root:

```sh
python3 -I -B scripts/capture-storage-query-oracle.py --output crates/librairii-storage/tests/fixtures/compatibility/prefix-queries.json
```

The capture refuses changed frozen internal Go sources or module inputs before
running Go. It opens only disposable copies of the manifest-hashed prefix DBs.
The result records the oracle commit, source-manifest hash, generator hash and
each input DB hash. Rust checks those identities before comparing actual returns
through its public owned database workers. The frozen Go application is neither
modified nor invoked by the normal Rust test suite.
