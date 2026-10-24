//! Retained Task 1 artwork workload: local reads and complete PNG decodes.
use std::time::Instant;

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
    for sample in 0..=SAMPLES {
        let cpu_start = process_cpu_seconds()?;
        let start = Instant::now();
        for path in &paths {
            let bytes = std::fs::read(path)?;
            let decoded = image::load_from_memory_with_format(&bytes, image::ImageFormat::Png)?;
            assert_eq!((decoded.width(), decoded.height()), (320, 400));
            // Consume every decoded byte; lazy header/thumbnail reads cannot pass.
            pixel_witness = std::hint::black_box(decoded.as_bytes())
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
        "artwork_probe; covers=24; size=320x400; samples_ms={durations:?}; p95_ms={p95:.6}; pixel_witness={pixel_witness}; cpu_samples_ms={cpu_durations:?}; method=read-full-decode-v2-fixed100"
    );
    fixture.close()?;
    println!("artwork_probe_complete; fixture_retired=true");
    if p95 > 50.0 {
        return Err("retained 24-read/full-PNG-decode p95 exceeds 50ms".into());
    }
    Ok(())
}
