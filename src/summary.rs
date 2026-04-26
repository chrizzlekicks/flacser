use std::path::PathBuf;

use crate::convert::{ExecutionReport, JobResult};

pub struct Summary {
    pub total: usize,
    pub converted: usize,
    pub skipped: usize,
    pub failed: usize,
    pub workers: usize,
    pub failure_details: Vec<(PathBuf, String)>,
}

impl Summary {
    pub fn from_report(report: &ExecutionReport) -> Self {
        let ExecutionReport { results, workers } = report;

        let mut converted = 0;
        let mut skipped = 0;
        let mut failed = 0;
        let mut failure_details = Vec::new();

        for result in results {
            match result {
                JobResult::Converted => converted += 1,
                JobResult::Skipped => skipped += 1,
                JobResult::Failed { input, error } => {
                    failed += 1;
                    failure_details.push((input.clone(), error.clone()));
                }
            }
        }

        Self {
            total: results.len(),
            converted,
            skipped,
            failed,
            workers: *workers,
            failure_details,
        }
    }

    pub fn print(&self) {
        println!(
            "Summary: total={}, converted={}, skipped={}, failed={}, workers={}",
            self.total, self.converted, self.skipped, self.failed, self.workers
        );

        for (input, error) in &self.failure_details {
            eprintln!("FAILED {}: {}", input.display(), error);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::convert::{ExecutionReport, JobResult};

    use super::Summary;

    #[test]
    fn aggregates_counts_and_failure_details() {
        let failed_input = PathBuf::from("broken.flac");
        let results = vec![
            JobResult::Converted,
            JobResult::Skipped,
            JobResult::Failed {
                input: failed_input.clone(),
                error: "ffmpeg failed".to_string(),
            },
        ];

        let report = ExecutionReport {
            results,
            workers: 2,
        };

        let summary = Summary::from_report(&report);
        assert_eq!(summary.total, 3);
        assert_eq!(summary.converted, 1);
        assert_eq!(summary.skipped, 1);
        assert_eq!(summary.failed, 1);
        assert_eq!(summary.workers, 2);
        assert_eq!(
            summary.failure_details,
            vec![(failed_input, "ffmpeg failed".to_string())]
        );
    }
}
