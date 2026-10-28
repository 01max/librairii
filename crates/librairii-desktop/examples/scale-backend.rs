//! Retained Task 1 workload through actual Rust database workers, never user data.
#[path = "scale-backend/cpu.rs"]
mod cpu;
#[path = "scale-backend/fixture.rs"]
mod fixture;
#[path = "scale-backend/queries.rs"]
mod queries;
#[path = "scale-backend/shelves.rs"]
mod shelves;

use librairii_storage::{IsolatedLibrary, database::Database};
use librairii_testkit::TestRoots;
use serde_json::json;
use std::{error::Error, path::Path, time::Instant};

type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;

fn main() -> Result {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    let shelves_only = match arguments.as_slice() {
        [] => false,
        [mode] if mode == "--shelves" => true,
        _ => return Err("usage: scale-backend [--shelves]".into()),
    };
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let roots = TestRoots::new()?;
    roots.authorize(Some(roots.library()), Some(roots.profile()))?;
    let library = IsolatedLibrary::initialize_from_override(
        Some(roots.library().as_os_str()),
        "a0000000-0000-4000-8000-000000000016".parse()?,
    )?;
    let mut database = Database::open_isolated(&library)?;
    let started = Instant::now();
    let (workload, count, fixture_ms, report) = if shelves_only {
        let oracle = shelves::seed(&database, &repository)?;
        let fixture_ms = started.elapsed().as_secs_f64() * 1000.;
        (
            "shelves",
            5000,
            fixture_ms,
            shelves::run(&database, &oracle)?,
        )
    } else {
        let rows = fixture::seed(&database, &repository, roots.library())?;
        let fixture_ms = started.elapsed().as_secs_f64() * 1000.;
        (
            "queries",
            rows.len(),
            fixture_ms,
            queries::run(&database, &rows, roots.library())?,
        )
    };
    database.close()?;
    drop(database);
    let retired_root = roots
        .library()
        .parent()
        .ok_or("missing fixture parent")?
        .to_owned();
    drop(library);
    drop(roots);
    if retired_root.exists() {
        return Err("owned fixture was not retired".into());
    }
    let passed = report["passed"].as_bool().unwrap_or(false);
    let mut payload = json!({
        "format": "librairii.scale-backend", "version": 1,
        "stories": count, "fixture_generation_ms": fixture_ms, "workload":workload,
        "method": "retained warmed wall samples and exact frozen outcomes; original budgets; no sample replacement",
        "fixture_retired": true, "database_closed": true
    });
    payload[workload] = report;
    println!("{}", serde_json::to_string_pretty(&payload)?);
    if !passed {
        return Err("retained backend performance gate failed; raw samples retained".into());
    }
    Ok(())
}
