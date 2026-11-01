//! Separate query-plan experiment, after all retained acceptance measurements.
//! These observations cannot replace, rescore or approve an acceptance sample.
use super::cpu;
use librairii_core::query::QueryRequest;
use librairii_storage::{StorageError, database::Database, query};
use rusqlite::{StatementStatus, Transaction, params_from_iter, types::Value};
use serde_json::{Value as Json, json};
use std::time::Instant;

fn bindings(plan: &query::StatementPlan) -> Vec<Value> {
    plan.parameters
        .iter()
        .map(|value| match value {
            query::QueryParameter::Integer(n) => Value::Integer(*n),
            query::QueryParameter::Text(s) => Value::Text(s.clone()),
        })
        .collect()
}

fn observe(
    tx: &Transaction<'_>,
    plan: &query::StatementPlan,
) -> librairii_storage::Result<(Vec<Vec<Value>>, Json)> {
    let cpu_start = cpu::milliseconds();
    let start = Instant::now();
    let mut statement = tx.prepare(&plan.statement)?;
    let columns = statement.column_count();
    let rows = statement
        .query_map(params_from_iter(bindings(plan)), |row| {
            (0..columns)
                .map(|column| row.get::<_, Value>(column))
                .collect::<rusqlite::Result<Vec<_>>>()
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let wall_ms = start.elapsed().as_secs_f64() * 1000.;
    let process_cpu_ms = cpu::milliseconds() - cpu_start;
    let observation = json!({"wall_ms": wall_ms, "process_cpu_ms": process_cpu_ms,
        "rows": rows.len(), "columns": columns,
        "vm_steps": statement.get_status(StatementStatus::VmStep),
        "fullscan_steps": statement.get_status(StatementStatus::FullscanStep),
        "sorts": statement.get_status(StatementStatus::Sort),
        "statement_memory_bytes": statement.get_status(StatementStatus::MemUsed)});
    Ok((rows, observation))
}

pub fn run(database: &Database<'_>) -> librairii_storage::Result<Json> {
    database
        .read(|tx| {
            let current = query::explain_page(tx, QueryRequest::default(), "en-GB")?
                .ok_or(StorageError::CompatibilityInvariant)?;
            // Reuse the actual existing deep-page implementation, changing only
            // OFFSET to zero for this experiment. Production search is unchanged.
            let mut candidate = query::explain_page(
                tx,
                QueryRequest {
                    page: 2,
                    ..Default::default()
                },
                "en-GB",
            )?
            .ok_or(StorageError::CompatibilityInvariant)?;
            if !candidate
                .statement
                .starts_with("WITH selected_page AS MATERIALIZED (SELECT s.id,")
            {
                return Err(StorageError::CompatibilityInvariant);
            }
            match candidate.parameters.last_mut() {
                Some(query::QueryParameter::Integer(offset)) if *offset == 24 => *offset = 0,
                _ => return Err(StorageError::CompatibilityInvariant),
            }
            // Explain the changed bindings too; do not label page-two observations
            // as the experimental first-page execution.
            candidate.steps = tx
                .prepare(&format!("EXPLAIN QUERY PLAN {}", candidate.statement))?
                .query_map(params_from_iter(bindings(&candidate)), |row| row.get(3))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            let (expected, current_warmup) = observe(tx, &current)?;
            let (alternative, candidate_warmup) = observe(tx, &candidate)?;
            if expected.len() != 24
                || expected.iter().any(|row| row.len() != 13)
                || alternative != expected
            {
                return Err(StorageError::CompatibilityInvariant);
            }
            let mut pairs = Vec::new();
            for index in 0..20 {
                let candidate_first = index % 2 == 1;
                let (first, second) = if candidate_first {
                    (&candidate, &current)
                } else {
                    (&current, &candidate)
                };
                let (first_rows, first_observation) = observe(tx, first)?;
                let (second_rows, second_observation) = observe(tx, second)?;
                if first_rows != expected || second_rows != expected {
                    return Err(StorageError::CompatibilityInvariant);
                }
                let (current_observation, candidate_observation) = if candidate_first {
                    (second_observation, first_observation)
                } else {
                    (first_observation, second_observation)
                };
                pairs.push(json!({"candidate_first": candidate_first,
                "current": current_observation, "candidate": candidate_observation,
                "all_13_columns_equal": true}));
            }
            Ok(
                json!({"format": "librairii.query-plan-experiment", "version": 1,
            "scope": "page SQL only; excludes count, summary enrichment and worker queue",
            "acceptance_evidence": false, "causal_fix_proven": false,
            "clock": "wall and process-wide CPU; not per-thread off-CPU time",
            "current_plan": current, "candidate_plan": candidate,
            "warmups": {"current": current_warmup, "candidate": candidate_warmup},
            "pairs": pairs}),
            )
        })?
        .wait()
}
