# Synthetic TLS fixture

Generated with local OpenSSL for Task 11 tests. `fixture.key.pem` is deliberately
public, synthetic test material; never deploy it or trust this CA outside tests.
The root CA signs a localhost/127.0.0.1 server certificate (CA:false, serverAuth),
valid for ten years from 2026-10-21. No private production credentials or network
capture are included. Tests prove unknown-root and wrong-name refusal, then use
the explicit fixture root for the positive local TLS case. The CA signing key is
not needed by the tests and is not retained in the repository.
