//! Temporary bounded in-memory observer timings. Never qualification evidence.
#[derive(Debug)]
pub struct Trace {
    rows: Vec<[u64; 10]>,
    limit: usize,
    pub overflowed: bool,
    frozen: bool,
}

impl Trace {
    pub fn new() -> Self {
        Self::with_limit(20_000)
    }

    fn with_limit(limit: usize) -> Self {
        Self {
            rows: Vec::with_capacity(limit),
            limit,
            overflowed: false,
            frozen: false,
        }
    }

    pub fn record(&mut self, row: [u64; 10]) {
        if self.frozen {
            return;
        }
        if self.rows.len() < self.limit {
            self.rows.push(row);
        } else {
            self.overflowed = true;
        }
    }

    /// Seal once, before shutdown work can contaminate input-cohort timings.
    pub fn freeze(&mut self) -> bool {
        if self.frozen {
            false
        } else {
            self.frozen = true;
            true
        }
    }

    pub fn rows(&self) -> &[[u64; 10]] {
        &self.rows
    }
}

#[cfg(test)]
mod tests {
    use super::Trace;

    #[test]
    fn overflow_retains_the_first_records_without_growing_or_selecting_samples() {
        let mut trace = Trace::with_limit(2);
        trace.record([1; 10]);
        trace.record([2; 10]);
        trace.record([3; 10]);
        assert_eq!(trace.rows(), &[[1; 10], [2; 10]]);
        assert!(trace.overflowed);
        assert_eq!(trace.rows.capacity(), 2);
    }

    #[test]
    fn shutdown_records_cannot_replace_or_extend_the_frozen_input_trace() {
        let mut trace = Trace::with_limit(2);
        trace.record([1; 10]);
        assert!(trace.freeze());
        trace.record([9; 10]);
        assert!(!trace.freeze());
        assert_eq!(trace.rows(), &[[1; 10]]);
        assert!(!trace.overflowed);
    }
}
