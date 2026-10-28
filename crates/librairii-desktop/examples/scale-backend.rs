//! Retained Task 1 workload through actual Rust database workers, never user data.
#[path = "scale-backend/fixture.rs"]
mod fixture;
#[path = "scale-backend/queries.rs"]
mod queries;

use librairii_storage::{IsolatedLibrary, database::Database};
use librairii_testkit::TestRoots;
use serde_json::json;
use std::{error::Error, path::Path, time::Instant};

type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;

fn main() -> Result {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let roots = TestRoots::new()?;
    roots.authorize(Some(roots.library()), Some(roots.profile()))?;
    let library = IsolatedLibrary::initialize_from_override(
        Some(roots.library().as_os_str()),
        "a0000000-0000-4000-8000-000000000016".parse()?,
    )?;
    let mut database = Database::open_isolated(&library)?;
    let started = Instant::now();
    let rows = fixture::seed(&database, &repository, roots.library())?;
    let fixture_ms = started.elapsed().as_secs_f64() * 1000.;
    let report = queries::run(&database, &rows, roots.library())?;
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
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "format": "librairii.scale-backend", "version": 1,
            "stories": rows.len(), "fixture_generation_ms": fixture_ms,
            "method": "one warmup; twenty query samples and one hundred full artwork batches; nearest-rank wall p95; original budgets",
            "fixture_retired": true, "database_closed": true, "queries": report
        }))?
    );
    if !report["passed"].as_bool().unwrap_or(false) {
        return Err("retained backend performance gate failed; raw samples retained".into());
    }
    Ok(())
}
