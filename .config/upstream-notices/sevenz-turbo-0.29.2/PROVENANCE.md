sevenz-turbo 0.29.2: pinned local decoder patch

Published archive: https://crates.io/api/v1/crates/sevenz-turbo/0.29.2/download
Archive SHA256: 110c472f8f3db3fb0f3a41d9e288e36bb754f3dc0475a6c549d7754cdca62bed
Upstream revision: 376143a85ddce67698df114eead4c702fef15e95
License: Apache-2.0; the exact upstream LICENSE remains in third_party/sevenz-turbo/LICENSE.

Librairii modifies src/reader.rs to decode encoded-header BCJ2 graphs through the
existing bounded graph decoder, applying aggregate memory admission and checked
packed-stream counts/ranges before constructing any decoder. src/codec/brotli.rs
reads the optional envelope completely, refuses the large-window extension before
allocation, and validates one Brotli stream without automatic frame concatenation.
These changes preserve the frozen Go import dialect and close the unbounded
large-window allocation path. They add no extraction or filesystem authority.

.config/sevenz-turbo-patch.json retains the original published 176-file index and
the two explicit patched file hashes. xtask/check_archive_patch.py verifies the
closed tree and both locked dependency graph overrides. Dependency inventories
record the current tree digest separately from the published archive checksum.
The retained package's examples, fixtures and guidance are upstream source data.
No product-wide license is selected by this notice.
