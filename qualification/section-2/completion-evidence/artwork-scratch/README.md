# Task 2.9 artwork scratch experiment

Run 68 at `f166d78` failed the unchanged 50 ms artwork wall-p95 gate on
Intel macOS. Its complete raw log and paired vectors are retained here with
artifact/source/binary hashes. Wall p95 was 61.131010 ms; process CPU p95 was
51.246 ms. Nine wall samples exceeded 50 ms, including six CPU samples. This
mixed trace does not establish a root cause or justify host settling alone.
The prior reviewed run-67 decision remains historical evidence. Tasks 2.9 and
2.13 stay open pending fresh native qualification.

The experiment reuses compressed/output scratch across covers while opening
every file and reading to EOF, creating a fresh pinned image 0.25.10 PNG decoder
with CRC checks, validating 320×400 RGB8 and exact output size, decoding the
complete image, and consuming every pixel byte. It never caches decoded results.
One warmup and all 100 measured batches, paired wall/CPU vectors, exact witness
118563263296, fixture retirement and the 50 ms wall-p95 ceiling are unchanged.
The distinct method is `read-full-decode-v3-scratch-fixed100`.

Three Rust regression tests compare every byte of all 24 retained covers against
the previous DynamicImage route while replacing the same input path, and reject
invalid PNG data, IDAT CRC corruption, truncated pixel streams, wrong dimensions,
wrong color type and short output buffers. All pass in a release CPU-only harness;
Clippy passes with warnings denied. The harness includes the previous probe from
`f166d78` and the current module, uses compiler 1.99.1 and the same 28 non-root
dependency versions/checksums as the qualification lock. It has no GUI initialization.

`local-comparison.json` retains a predeclared old/current/current/old release
comparison on this ARM Mac with no concurrent agent compiler or regression tests.
All four complete vectors/witnesses pass; p95 values overlap closely (8.86–9.25 ms).
This supports output equivalence and does **not** demonstrate repair of the
sporadic Intel CPU spikes. `local-comparison-with-clippy-overlap.json` retains an
earlier diagnostic whose first old run overlapped a 2.66-second Clippy check; it
is not a controlled performance comparison. Neither local comparison is packaged
qualification or a replacement for fresh ARM Mac, Intel Mac and Linux gates.
