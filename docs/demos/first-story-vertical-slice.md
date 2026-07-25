# First Story Vertical Slice

This demonstration exercises the first complete local workflow using only the
copyright-free synthetic archive checked into the repository. It does not
require a Lunii account, a storyteller device, or copyrighted story bytes.

## Automated demonstration

From the repository root, run:

```sh
make smoke-first-story
```

The test composes the same application, import runtime, SQLite repositories,
archive custody, library queries, and removal service used by the packaged
application. It selects a synthetic archive through the dialog port, imports
it, lists and inspects the story, moves the managed copy to application trash,
and verifies that the selected source bytes remain unchanged.

## Packaged application walkthrough

1. Build the current-platform application with `make build`.
2. Start it with an isolated data root. On macOS:

   ```sh
   LIBRAIRII_DATA_ROOT="$(mktemp -d)/librairii" \
     build/bin/Librairii.app/Contents/MacOS/Librairii
   ```

3. Choose **Import stories** and select
   `internal/inspection/testfixture/testdata/generic.7z`.
4. Observe the nonblocking import state and the new selected cover on the
   collection shelf.
5. Choose **Open details** to inspect its UUID, detected format, verification,
   filename, and checksum.
6. Choose **Cancel** and verify the story remains.
7. Open details again, choose **Move to trash**, and verify the collection
   returns to its empty state.

The checked-in `generic.7z` fixture is deterministic and synthetic. Import and
removal never modify it; Librairii operates on its managed copy, and confirmed
removal leaves that managed copy under the isolated data root's `trash`
directory.
