use librairii_core::Checksum;
use librairii_storage::{
    StorageError,
    database::WorkCancellation,
    platform::{HASH_BUFFER_BYTES, hash_stream},
};
use std::{
    error::Error,
    io::{self, Read},
};

type TestResult = Result<(), Box<dyn Error>>;

struct Fragmented<R> {
    inner: R,
    fragment: usize,
    largest_request: usize,
    calls: usize,
}
impl<R: Read> Read for Fragmented<R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.largest_request = self.largest_request.max(buffer.len());
        self.calls += 1;
        let length = buffer.len().min(self.fragment);
        self.inner.read(&mut buffer[..length])
    }
}

#[test]
fn complete_stream_hash_matches_nist_vectors_across_fixed_buffer_boundaries() -> TestResult {
    let cancellation = WorkCancellation::default();
    for (mut data, expected) in [
        (
            b"".as_slice(),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        ),
        (
            b"abc",
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        ),
    ] {
        let length = data.len() as u64;
        let measured = hash_stream(&mut data, length, &cancellation)?;
        assert_eq!(measured.checksum(), expected.parse::<Checksum>()?);
        assert_eq!(measured.bytes(), length);
    }
    for fragment in [63, 64, 65, 4096, 65536, 65537] {
        let mut input = Fragmented {
            inner: io::repeat(b'a').take(1_000_000),
            fragment,
            largest_request: 0,
            calls: 0,
        };
        let measured = hash_stream(&mut input, 1_000_000, &cancellation)?;
        assert_eq!(
            measured.checksum().to_string(),
            "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0"
        );
        assert_eq!(measured.bytes(), 1_000_000);
        assert!(input.largest_request <= HASH_BUFFER_BYTES);
        assert!(input.calls > 1);
    }
    Ok(())
}

#[test]
fn stream_size_ceiling_is_exact_and_consumes_at_most_one_sentinel_byte() -> TestResult {
    let cancellation = WorkCancellation::default();
    for limit in [0, 1, 65535, 65536, 65537] {
        let mut exact = io::repeat(0x5a).take(limit);
        assert_eq!(
            hash_stream(&mut exact, limit, &cancellation)?.bytes(),
            limit
        );
        let mut overflow = io::repeat(0x5a).take(limit + 2);
        assert!(matches!(
            hash_stream(&mut overflow, limit, &cancellation),
            Err(StorageError::HashSizeLimit)
        ));
        assert_eq!(overflow.limit(), 1);
    }
    assert_eq!(
        hash_stream(&mut b"abc".as_slice(), u64::MAX, &cancellation)?.bytes(),
        3
    );
    Ok(())
}

struct ReadFault {
    remaining: usize,
    consumed: usize,
}
impl Read for ReadFault {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if self.remaining == 0 {
            return Err(io::Error::other("injected source read failure"));
        }
        let count = self.remaining.min(buffer.len());
        buffer[..count].fill(b'x');
        self.remaining -= count;
        self.consumed += count;
        Ok(count)
    }
}

#[test]
fn read_failure_after_real_chunks_returns_cause_without_partial_checksum() -> TestResult {
    let mut reader = ReadFault {
        remaining: HASH_BUFFER_BYTES + 17,
        consumed: 0,
    };
    let result = hash_stream(&mut reader, u64::MAX, &WorkCancellation::default());
    match result {
        Err(StorageError::Io { operation, source }) => {
            assert_eq!(operation, "hash stream");
            assert_eq!(source.kind(), io::ErrorKind::Other);
            assert_eq!(source.to_string(), "injected source read failure");
        }
        other => return Err(format!("unexpected read-error outcome: {other:?}").into()),
    }
    assert_eq!(reader.consumed, HASH_BUFFER_BYTES + 17);
    Ok(())
}

struct CancelReader {
    cancellation: WorkCancellation,
    calls: usize,
    error: bool,
}
impl Read for CancelReader {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.calls += 1;
        self.cancellation.cancel();
        if self.error {
            return Err(io::Error::other(
                "read failed with independent cancellation",
            ));
        }
        buffer.fill(b'a');
        Ok(buffer.len())
    }
}

#[test]
fn streaming_cancel_stops_before_next_read_without_claiming_complete_digest() -> TestResult {
    let cancellation = WorkCancellation::default();
    cancellation.cancel();
    let mut reader = CancelReader {
        cancellation: cancellation.clone(),
        calls: 0,
        error: false,
    };
    assert!(matches!(
        hash_stream(&mut reader, u64::MAX, &cancellation),
        Err(StorageError::Cancelled)
    ));
    assert_eq!(reader.calls, 0);
    let cancellation = WorkCancellation::default();
    let mut reader = CancelReader {
        cancellation: cancellation.clone(),
        calls: 0,
        error: false,
    };
    assert!(matches!(
        hash_stream(&mut reader, u64::MAX, &cancellation),
        Err(StorageError::Cancelled)
    ));
    assert_eq!(reader.calls, 1);
    Ok(())
}

#[test]
fn independent_read_error_is_not_erased_when_cancel_arrives_during_read() -> TestResult {
    let cancellation = WorkCancellation::default();
    let mut reader = CancelReader {
        cancellation: cancellation.clone(),
        calls: 0,
        error: true,
    };
    assert!(matches!(
        hash_stream(&mut reader, u64::MAX, &cancellation),
        Err(StorageError::Io { .. })
    ));
    assert!(cancellation.is_cancelled());
    assert_eq!(reader.calls, 1);
    Ok(())
}

#[test]
fn interrupted_reads_resume_but_invalid_read_count_never_panics_or_yields_digest() -> TestResult {
    struct Interrupted {
        remaining: usize,
        bytes: &'static [u8],
    }
    impl Read for Interrupted {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            if self.remaining > 0 {
                self.remaining -= 1;
                return Err(io::ErrorKind::Interrupted.into());
            }
            self.bytes.read(buffer)
        }
    }
    let mut reader = Interrupted {
        remaining: 3,
        bytes: b"abc",
    };
    let cancellation = WorkCancellation::default();
    assert_eq!(
        hash_stream(&mut reader, u64::MAX, &cancellation)?
            .checksum()
            .to_string(),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    struct InvalidCount;
    impl Read for InvalidCount {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            Ok(usize::MAX)
        }
    }
    assert!(
        matches!(hash_stream(&mut InvalidCount, u64::MAX, &cancellation), Err(StorageError::Io { source, .. }) if source.kind() == io::ErrorKind::InvalidData)
    );
    Ok(())
}
