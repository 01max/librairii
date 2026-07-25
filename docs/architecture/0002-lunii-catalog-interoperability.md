# Lunii catalog interoperability boundary

Status: accepted for `rebuild-local-story-library-wails`

Librairii's official-metadata scope is defined only by the active OpenSpec
change. The wire-level endpoint names, guest-token envelope, catalog header
names, and catalog envelope were independently reimplemented from observable
behavior in [Lunii.QT](https://github.com/o-daneel/Lunii.QT) at tree
`c8afe43dde21c2be33c0667ced962dc023eb948a`. Lunii.QT is licensed GPL-3.0;
none of its source code or copyrighted catalog data is copied into Librairii.

`internal/lunii` is the only package that knows:

- the guest-token and pack-catalog URLs;
- `Application-Sender` and `X-AUTH-TOKEN`;
- response envelopes, limits, and transport deadlines; and
- the sanitized, synthetic contract fixtures.

The adapter exposes no username, password, cookie, account token, device key,
firmware, archive-download, or user-data path. It uses Go's verified TLS stack
with a TLS 1.2 minimum and fetches only the public metadata catalog. Artwork is
handled later through its own bounded, read-only adapter.
