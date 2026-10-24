//! Retained Task 1 artwork workload: local reads and complete PNG decodes.
use image::{ImageDecoder, codecs::png::PngDecoder};
use std::{io::Read, path::Path, time::Instant};

const PIXEL_BYTES: usize = 320 * 400 * 3;

fn decode_into(bytes: &[u8], pixels: &mut [u8]) -> Result<(), Box<dyn std::error::Error>> {
    // A fresh decoder retains the pinned PNG implementation and its CRC checks.
    let decoder = PngDecoder::new(std::io::Cursor::new(bytes))?;
    if decoder.dimensions() != (320, 400)
        || decoder.color_type() != image::ColorType::Rgb8
        || decoder.total_bytes() != PIXEL_BYTES as u64
        || pixels.len() != PIXEL_BYTES
    {
        return Err("retained artwork must fully decode to 320x400 RGB8".into());
    }
    decoder.read_image(pixels)?;
    Ok(())
}

struct Scratch {
    encoded: Vec<u8>,
    pixels: Vec<u8>,
}

impl Scratch {
    fn new(encoded_capacity: usize) -> Self {
        Self {
            encoded: Vec::with_capacity(encoded_capacity),
            pixels: vec![0; PIXEL_BYTES],
        }
    }

    fn read_decode(&mut self, path: &Path) -> Result<&[u8], Box<dyn std::error::Error>> {
        self.encoded.clear();
        // Every cover is reopened, read to EOF and decoded again, even in warm batches.
        std::fs::File::open(path)?.read_to_end(&mut self.encoded)?;
        decode_into(&self.encoded, &mut self.pixels)?;
        Ok(&self.pixels)
    }
}

#[cfg(unix)]
fn process_cpu_seconds() -> std::io::Result<f64> {
    let mut value = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: value is an initialized, correctly aligned writable timespec.
    // This public clock reads only the current process's accumulated CPU time.
    if unsafe { libc::clock_gettime(libc::CLOCK_PROCESS_CPUTIME_ID, &raw mut value) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(value.tv_sec as f64 + value.tv_nsec as f64 / 1_000_000_000.0)
}

#[cfg(not(unix))]
fn process_cpu_seconds() -> std::io::Result<f64> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "artwork CPU diagnostics require a qualified Unix target",
    ))
}

macro_rules! cover {
    ($n:literal) => {
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../preservation/corpus/performance/catalog/embedded/performance-cover-",
            $n,
            ".png"
        )) as &[u8]
    };
}

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let covers = [
        cover!("01"),
        cover!("02"),
        cover!("03"),
        cover!("04"),
        cover!("05"),
        cover!("06"),
        cover!("07"),
        cover!("08"),
        cover!("09"),
        cover!("10"),
        cover!("11"),
        cover!("12"),
        cover!("13"),
        cover!("14"),
        cover!("15"),
        cover!("16"),
        cover!("17"),
        cover!("18"),
        cover!("19"),
        cover!("20"),
        cover!("21"),
        cover!("22"),
        cover!("23"),
        cover!("24"),
    ];
    let fixture = tempfile::tempdir()?;
    let paths = covers
        .iter()
        .enumerate()
        .map(|(index, bytes)| {
            let path = fixture.path().join(format!("cover-{index:02}.png"));
            std::fs::write(&path, bytes)?;
            Ok(path)
        })
        .collect::<Result<Vec<_>, std::io::Error>>()?;
    const SAMPLES: usize = 100;
    let mut durations = Vec::new();
    let mut cpu_durations = Vec::new();
    let mut pixel_witness = 0u64;
    // Reuse bounded fixture scratch, never decoded-image results. The warmup and
    // every measured batch still include all 24 reads, decodes and pixel visits.
    let mut scratch = Scratch::new(covers.iter().map(|cover| cover.len()).max().unwrap());
    for sample in 0..=SAMPLES {
        let cpu_start = process_cpu_seconds()?;
        let start = Instant::now();
        for path in &paths {
            let decoded = scratch.read_decode(path)?;
            // Consume every decoded byte; lazy header/thumbnail reads cannot pass.
            pixel_witness = std::hint::black_box(decoded)
                .iter()
                .fold(pixel_witness, |sum, value| {
                    sum.wrapping_add(u64::from(*value))
                });
        }
        let ms = start.elapsed().as_secs_f64() * 1000.0;
        let cpu_ms = (process_cpu_seconds()? - cpu_start) * 1000.0;
        assert!(cpu_ms.is_finite() && cpu_ms >= 0.0);
        if sample > 0 {
            durations.push(ms);
            cpu_durations.push(cpu_ms);
        } else {
            println!("artwork_warmup; wall_ms={ms:.6}; cpu_ms={cpu_ms:.6}");
        }
    }
    let mut ordered = durations.clone();
    ordered.sort_by(f64::total_cmp);
    let p95 = ordered[(SAMPLES * 95).div_ceil(100) - 1];
    println!(
        "artwork_probe; covers=24; size=320x400; samples_ms={durations:?}; p95_ms={p95:.6}; pixel_witness={pixel_witness}; cpu_samples_ms={cpu_durations:?}; method=read-full-decode-v3-scratch-fixed100"
    );
    fixture.close()?;
    println!("artwork_probe_complete; fixture_retired=true");
    if p95 > 50.0 {
        return Err("retained 24-read/full-PNG-decode p95 exceeds 50ms".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::ImageEncoder;

    #[test]
    fn scratch_reopens_files_and_matches_every_dynamic_image_byte() {
        let fixture = tempfile::tempdir().unwrap();
        let path = fixture.path().join("replaced.png");
        let mut scratch = Scratch::new(0);
        let mut previous = Vec::new();
        for index in 1..=24 {
            let bytes = std::fs::read(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../../preservation/corpus/performance/catalog/embedded")
                    .join(format!("performance-cover-{index:02}.png")),
            )
            .unwrap();
            let original =
                image::load_from_memory_with_format(&bytes, image::ImageFormat::Png).unwrap();
            std::fs::write(&path, &bytes).unwrap();
            let actual = scratch.read_decode(&path).unwrap();
            assert_eq!(actual, original.as_bytes());
            assert_ne!(actual, previous);
            previous = actual.to_vec();
        }
        std::fs::write(&path, b"not a PNG").unwrap();
        assert!(scratch.read_decode(&path).is_err());
    }

    #[test]
    fn rejects_idat_crc_corruption_and_truncated_pixel_stream() {
        let original = cover!("01");
        let mut corrupt = original.to_vec();
        let mut offset = 8;
        loop {
            let length =
                u32::from_be_bytes(corrupt[offset..offset + 4].try_into().unwrap()) as usize;
            if &corrupt[offset + 4..offset + 8] == b"IDAT" {
                corrupt[offset + 8 + length] ^= 1; // CRC only; pixels remain otherwise valid.
                break;
            }
            offset += 12 + length;
        }
        let mut pixels = vec![0; PIXEL_BYTES];
        assert!(decode_into(&corrupt, &mut pixels).is_err());
        assert!(decode_into(&original[..original.len() / 2], &mut pixels).is_err());
    }

    #[test]
    fn rejects_wrong_dimensions_color_and_short_output() {
        for (width, color) in [
            (319, image::ExtendedColorType::Rgb8),
            (320, image::ExtendedColorType::Rgba8),
        ] {
            let mut encoded = Vec::new();
            let bytes = vec![0; width as usize * 400 * color.channel_count() as usize];
            image::codecs::png::PngEncoder::new(&mut encoded)
                .write_image(&bytes, width, 400, color)
                .unwrap();
            assert!(decode_into(&encoded, &mut vec![0; PIXEL_BYTES]).is_err());
        }
        assert!(decode_into(cover!("01"), &mut vec![0; PIXEL_BYTES - 1]).is_err());
    }
}
