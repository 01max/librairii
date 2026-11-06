// Modified by Librairii: strict windows, exact envelope reads and single-stream validation.
#[cfg(feature = "compress")]
use std::io::Write;
use std::io::{self, Cursor, Read};

// The frozen Go decoder uses strict RFC7932 windows (at most 24 bits).
// Refuse the large-window extension before creating any decompressor, including
// the optional zstdmt envelope. Its unrestricted allocator is otherwise unbounded
// by sevenz-turbo's fixed 32MiB model.
fn strict_window(first: u8) -> io::Result<()> {
    if first & 0x7F == 0x11 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "large-window Brotli is unsupported",
        ));
    }
    Ok(())
}

use crate::{ByteReader, Error};

/// Magic bytes of a skippable frame format as used in brotli by zstdmt.
const SKIPPABLE_FRAME_MAGIC: u32 = 0x184D2A50;
/// "BR" in little-endian
const BROTLI_MAGIC: u16 = 0x5242;
#[cfg(feature = "compress")]
const HINT_UNIT_SIZE: usize = 65536;

/// Custom decoder to support the custom format first implemented by zstdmt, which allows to have
/// optional skippable frames.
pub(crate) struct BrotliDecoder<R: Read> {
    inner: Option<StrictDecoder<InnerReader<R>>>,
    pending: Option<InnerReader<R>>,
    buffer_size: usize,
}

impl<R: Read> BrotliDecoder<R> {
    pub(crate) fn new(mut input: R, buffer_size: usize) -> Result<Self, Error> {
        let mut header = [0u8; 16];
        input.read_exact(&mut header)?;
        // Match the frozen Go wrapper: strip its recognized 16-byte envelope,
        // otherwise replay those bytes. The envelope's hints are not authority
        // to allocate or to concatenate additional Brotli streams.
        let framed = u32::from_le_bytes([header[0], header[1], header[2], header[3]])
            == SKIPPABLE_FRAME_MAGIC
            && u32::from_le_bytes([header[4], header[5], header[6], header[7]]) == 8
            && Cursor::new(&header[12..14]).read_u16()? == BROTLI_MAGIC;
        let inner_reader = if framed {
            InnerReader::new_standard(input, Vec::new())
        } else {
            InnerReader::new_standard(input, header.to_vec())
        };

        Ok(BrotliDecoder {
            inner: None,
            pending: Some(inner_reader),
            buffer_size,
        })
    }
}

impl<R: Read> Read for BrotliDecoder<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        if self.inner.is_none() {
            let Some(mut pending) = self.pending.take() else {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "missing Brotli input",
                ));
            };
            // Match Go's read-time error boundary, before any decoder allocation.
            if let Err(error) = pending.check_window() {
                self.pending = Some(pending);
                return Err(error);
            }
            self.inner = Some(StrictDecoder::new(pending, self.buffer_size));
        }
        match self.inner.as_mut() {
            Some(reader) => reader.read(buf),
            None => Ok(0),
        }
    }
}

// The upstream Read wrapper delays excessive-input errors until *after* an EOF
// read. Import entries have declared lengths and need the error on the read that
// completes the stream, matching Go. Use the same streaming engine directly;
// never buffer decoded output or concatenate independent streams.
struct StrictDecoder<R: Read> {
    input: R,
    buffer: Vec<u8>,
    offset: usize,
    length: usize,
    total: usize,
    state: brotli::BrotliState<
        brotli::HeapAlloc<u8>,
        brotli::HeapAlloc<u32>,
        brotli::HeapAlloc<brotli::HuffmanCode>,
    >,
    done: bool,
    failed: bool,
}
impl<R: Read> StrictDecoder<R> {
    fn new(input: R, size: usize) -> Self {
        Self {
            input,
            buffer: vec![0; size.max(1)],
            offset: 0,
            length: 0,
            total: 0,
            state: brotli::BrotliState::new(
                brotli::HeapAlloc::default(),
                brotli::HeapAlloc::default(),
                brotli::HeapAlloc::default(),
            ),
            done: false,
            failed: false,
        }
    }
    fn invalid(&mut self, message: &'static str) -> io::Result<usize> {
        self.failed = true;
        Err(io::Error::new(io::ErrorKind::InvalidData, message))
    }
}
impl<R: Read> Read for StrictDecoder<R> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if self.failed {
            return self.invalid("failed Brotli stream");
        }
        if self.done || output.is_empty() {
            return Ok(0);
        }
        let mut written = 0;
        loop {
            let mut available_in = self.length - self.offset;
            let mut available_out = output.len() - written;
            let result = brotli::BrotliDecompressStream(
                &mut available_in,
                &mut self.offset,
                &self.buffer[..self.length],
                &mut available_out,
                &mut written,
                output,
                &mut self.total,
                &mut self.state,
            );
            match result {
                brotli::BrotliResult::ResultSuccess => {
                    // A packed stream is bounded by its declared compressed size.
                    // A second envelope/stream is excessive input, never a new frame.
                    let mut extra = [0u8; 1];
                    if available_in != 0 || self.input.read(&mut extra)? != 0 {
                        return self.invalid("excessive Brotli input");
                    }
                    self.done = true;
                    return Ok(written);
                }
                brotli::BrotliResult::ResultFailure => {
                    return self.invalid("invalid Brotli stream");
                }
                brotli::BrotliResult::NeedsMoreOutput => return Ok(written),
                brotli::BrotliResult::NeedsMoreInput => {
                    if written != 0 {
                        return Ok(written);
                    }
                    if available_in != 0 {
                        return self.invalid("invalid Brotli input state");
                    }
                    self.offset = 0;
                    self.length = self.input.read(&mut self.buffer)?;
                    if self.length == 0 {
                        return self.invalid("truncated Brotli stream");
                    }
                }
            }
        }
    }
}

struct InnerReader<R: Read> {
    reader: R,
    header_buffer: Cursor<Vec<u8>>,
}
impl<R: Read> InnerReader<R> {
    fn check_window(&mut self) -> io::Result<()> {
        if self.header_buffer.get_ref().is_empty() {
            let mut first = [0u8; 1];
            self.reader.read_exact(&mut first)?;
            self.header_buffer = Cursor::new(first.to_vec());
        }
        strict_window(self.header_buffer.get_ref()[0])
    }
    fn new_standard(reader: R, header: Vec<u8>) -> Self {
        Self {
            reader,
            header_buffer: Cursor::new(header),
        }
    }
}
impl<R: Read> Read for InnerReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let count = self.header_buffer.read(buf)?;
        if count != 0 {
            return Ok(count);
        }
        self.reader.read(buf)
    }
}

/// Custom encoder to support the custom format first implemented by zstdmt, which allows to have
/// optional skippable frames.
#[cfg(feature = "compress")]
pub(crate) struct BrotliEncoder<W: Write> {
    inner: InnerWriter<W>,
    quality: u32,
    window: u32,
    buffer_size: usize,
}

#[cfg(feature = "compress")]
enum InnerWriter<W: Write> {
    Standard(brotli::CompressorWriter<W>),
    Framed {
        writer: W,
        compressor: Option<brotli::CompressorWriter<Vec<u8>>>,
        frame_size: usize,
        uncompressed_bytes_in_frame: usize,
    },
}

#[cfg(feature = "compress")]
impl<W: Write> BrotliEncoder<W> {
    pub(crate) fn new(
        writer: W,
        quality: u32,
        window: u32,
        frame_size: usize,
    ) -> Result<Self, Error> {
        let buffer_size = 8192;

        let inner = if frame_size == 0 {
            let compressor = brotli::CompressorWriter::new(writer, buffer_size, quality, window);
            InnerWriter::Standard(compressor)
        } else {
            let compressor = Some(brotli::CompressorWriter::new(
                Vec::with_capacity(frame_size),
                buffer_size,
                quality,
                window,
            ));
            InnerWriter::Framed {
                writer,
                compressor,
                frame_size,
                uncompressed_bytes_in_frame: 0,
            }
        };

        Ok(Self {
            inner,
            quality,
            window,
            buffer_size,
        })
    }

    #[cfg(feature = "compress")]
    fn write_frame(
        writer: &mut W,
        compressed_data: &[u8],
        uncompressed_bytes: usize,
    ) -> io::Result<()> {
        use crate::ByteWriter;

        if compressed_data.is_empty() {
            return Ok(());
        }

        writer.write_u32(SKIPPABLE_FRAME_MAGIC)?;
        writer.write_u32(8)?;
        writer.write_u32(compressed_data.len() as u32)?;
        writer.write_u16(BROTLI_MAGIC)?;

        let hint_value = uncompressed_bytes.div_ceil(HINT_UNIT_SIZE);
        let hint_value = if hint_value > usize::from(u16::MAX) {
            u16::MAX
        } else {
            hint_value as u16
        };
        writer.write_u16(hint_value)?;

        writer.write_all(compressed_data)?;

        Ok(())
    }
}

#[cfg(feature = "compress")]
impl<W: Write> Write for BrotliEncoder<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match &mut self.inner {
            InnerWriter::Standard(compressor) => compressor.write(buf),
            InnerWriter::Framed {
                writer,
                compressor,
                frame_size,
                uncompressed_bytes_in_frame,
            } => {
                let mut bytes_consumed = 0;
                let total_bytes = buf.len();

                while bytes_consumed < total_bytes {
                    let comp = compressor.as_mut().expect("no compressor set");

                    let end = std::cmp::min(
                        total_bytes,
                        bytes_consumed + (*frame_size - *uncompressed_bytes_in_frame),
                    );
                    let chunk = &buf[bytes_consumed..end];
                    let bytes_written = comp.write(chunk)?;

                    if bytes_written == 0 && !chunk.is_empty() {
                        return Err(io::Error::new(
                            io::ErrorKind::WriteZero,
                            "failed to write any bytes",
                        ));
                    }

                    bytes_consumed += bytes_written;
                    *uncompressed_bytes_in_frame += bytes_written;

                    if *uncompressed_bytes_in_frame >= *frame_size {
                        let mut comp = compressor.take().expect("no compressor set");
                        comp.flush()?;
                        let mut data = comp.into_inner();

                        Self::write_frame(writer, &data, *uncompressed_bytes_in_frame)?;
                        data.clear();

                        *compressor = Some(brotli::CompressorWriter::new(
                            data,
                            self.buffer_size,
                            self.quality,
                            self.window,
                        ));

                        *uncompressed_bytes_in_frame = 0;
                    }
                }

                Ok(total_bytes)
            }
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match &mut self.inner {
            InnerWriter::Standard(compressor) => compressor.flush(),
            InnerWriter::Framed {
                writer,
                compressor,
                uncompressed_bytes_in_frame,
                ..
            } => {
                let mut comp = compressor.take().expect("no compressor set");
                comp.flush()?;
                let mut data = comp.into_inner();

                if !data.is_empty() {
                    Self::write_frame(writer, &data, *uncompressed_bytes_in_frame)?;
                    data.clear();
                    *uncompressed_bytes_in_frame = 0;
                }

                *compressor = Some(brotli::CompressorWriter::new(
                    data,
                    self.buffer_size,
                    self.quality,
                    self.window,
                ));

                writer.flush()
            }
        }
    }
}
