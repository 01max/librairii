# Frozen metadata/artwork observations

The probes execute real frozen Go `73a93303a9a44cf0a92eaf525331561ed8c04460`
with Go 1.26.5 in a separate `git archive`, `GOTOOLCHAIN=local`, `GOPROXY=off`.
Copy the probes as `internal/metadata/task11_reference_test.go` and
`internal/artwork/task11_reference_test.go`. Run the matching `TestTask11Frozen*`
test with `-count=1` and absolute `TASK11_CASES` / `TASK11_CAPTURE` or
`TASK11_ARTWORK_CASES` / `TASK11_ARTWORK_CAPTURE` paths. Two independent executions
produced byte-identical outputs. Routine Rust checks only read these files; they
never regenerate Go observations or the existing preservation corpus.

Catalog cases compare every normalized field, locale choice, UTC timestamp,
artwork identity/URL and accept/reject result. Artwork cases use the real Go
publisher and validated loader, comparing signature acceptance, bytes/hash,
length/media and the actual nested path. The full frozen metadata/artwork/Lunii
suites also pass with local transports only. Neither probe has production URLs
as request destinations; production URL strings are data for pure validation.

Six catalog inputs retain Go's permissive result and explicitly test a stricter
Rust rejection: invalid UTF-8, lone surrogate, deep unknown JSON, duplicate keys,
ambiguous struct spellings, and an unrepresentable unknown JSON number. These are
strict validation/resource policies at the optional refresh edge, preserving the
last valid snapshot. They are recorded differences, not claimed parity. The new
native decoder additionally bounds raster dimensions/pixels; malformed headers
that Go's signature-only artwork storage accepts can still become a placeholder.

The synthetic cover is copied from the preserved performance corpus; its source
is recorded in the manifest. No preservation file is modified.
