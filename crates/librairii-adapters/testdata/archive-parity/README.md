# Actual import codec and hardening corpus

Original synthetic inputs, with no production stories or copied upstream fixture.
`generate.py` and `synthetic_7z.py` produce bounded independent 7z framing. The
latter adapts our original qualification writer at41e8854; its format reference
and digest remain in its header. Official7-Zip26.05 creates the standard codecs
and filters; the pinned existing Go libraries create independent Zstd/Brotli/LZ4
streams. `codec-producer.go.txt` is the retained producer source: copy it to
`target/task5-codec-producer/main.go` before regeneration, then run:

```
TASK5_GO=/opt/homebrew/bin/go TASK5_7ZZ=/path/to/7zz python3 generate.py
```

The tool directory must retain the downloaded `archive.tar.xz` for its digest.
Tool URL/digest and input hashes appear in `manifest.json`. The executable is a
test producer only; no helper process is used to inspect production archives.
No upstream 7-Zip source/binary ships in Librairii. Tool terms: LGPL-2.1-or-later,
BSD portions and unRAR restriction as described in its supplied License.txt;
only new synthetic output is retained. Existing Go codec dependencies keep their
own module notices; none of their source is copied here.

`manifest.json` retains observed Go outcomes and expected Rust outcomes.
`hardening.json` separately records every intentionally stricter category.
Malformed maximum-count inputs are deliberately never sent to the unbounded Go
constructor. Native Rust constructors reject those before count-driven reserves.
Header counterexamples are valid bounded17MiB data; no hostile decoder is run
without its production constructor ceilings. A2GiB logical archive expansion cap
is per traversal, not a2GiB cumulative budget across repeated metadata requests.
Only the selected block is decoded; a solid predecessor is drained in32KiB
chunks with cancellation checks. Unrelated block payload CRCs remain unobserved.

The pinned decoder repair adds `extend.py`, which preserves the base corpus and
produces seven bounded codec/limit regressions with the same tool environment.
Re-run the retained Go probe on those exact bytes to refresh its observations;
the producer alone does not establish Go acceptance. The probe source is retained
at ../import-hardening/probe.go.txt and runs from an owned target directory.
GOMAXPROCS=1 keeps the known 256 MiB Zstd window to one Go decoder worker.
`patch-regressions.py` separately creates two tiny encoded-header constructor
regressions (packed-stream count mismatch and aggregate Zstd memory). Those
malformed declarations are tested only through the bounded Rust constructor.

Brotli standard/enveloped and encoded-header BCJ2 are required Accepted cases.
Large-window Brotli and concatenated streams preserve actual Go rejection reasons.
They are not used to waive ordinary codec support. The two additional hardening
records separately cover the 128 MiB Zstd window clamp and 10,000 packed-stream
ceiling. The many-stream fixture contains 3,000 valid files/coders and 12,000 packed
streams; official 7zz tests all files. Its harmless PackPos padding aligns codec IDs
to avoid frozen Go's unrelated short-Read method-ID parser defect. No Go code is
changed and all archive/header CRCs are recomputed over the retained bytes.
