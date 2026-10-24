//! Retained Task 1 artwork workload: local reads and complete PNG decodes.
use std::time::Instant;

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
    let mut durations = Vec::new();
    let mut pixel_witness = 0u64;
    for sample in 0..21 {
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
        if sample > 0 {
            durations.push(ms);
        }
    }
    let mut ordered = durations.clone();
    ordered.sort_by(f64::total_cmp);
    let p95 = ordered[18]; // nearest rank ceil(.95 * 20), zero based
    println!(
        "artwork_probe; covers=24; size=320x400; samples_ms={durations:?}; p95_ms={p95:.6}; pixel_witness={pixel_witness}"
    );
    fixture.close()?;
    println!("artwork_probe_complete; fixture_retired=true");
    if p95 > 50.0 {
        return Err("retained 24-read/full-PNG-decode p95 exceeds 50ms".into());
    }
    Ok(())
}
