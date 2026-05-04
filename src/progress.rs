use std::sync::Mutex;

/// Tracks and reports completed conversion jobs.
#[derive(Debug)]
pub struct ProgressReporter {
    total: usize,
    done: Mutex<usize>,
}

impl ProgressReporter {
    pub fn new(total: usize) -> Self {
        Self {
            total,
            done: Mutex::new(0),
        }
    }

    pub fn finish_job(&self) {
        let mut completed = self.done.lock().expect("progress reporter mutex poisoned");
        *completed += 1;
        println!("[{}/{}] done", *completed, self.total)
    }
}

#[cfg(test)]
mod tests {
    use super::ProgressReporter;

    #[test]
    fn new_starts_with_total_and_zero_done() {
        let reporter = ProgressReporter::new(3);

        assert_eq!(reporter.total, 3);
        assert_eq!(*reporter.done.lock().expect("lock progress state"), 0);
    }

    #[test]
    fn job_done_increments_completed_count() {
        let reporter = ProgressReporter::new(2);

        reporter.finish_job();
        reporter.finish_job();

        assert_eq!(*reporter.done.lock().expect("lock progress state"), 2);
    }
}
