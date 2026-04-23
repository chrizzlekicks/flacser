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

pub fn from_report(report: &ExecutionReport) -> Summary {
    let ExecutionReport { results, workers } = report;

    let mut summary = Summary {
        total: results.len(),
        converted: 0,
        skipped: 0,
        failed: 0,
        workers: *workers,
        failure_details: Vec::new(),
    };

    for result in results {
        match result {
            JobResult::Converted => summary.converted += 1,
            JobResult::Skipped => summary.skipped += 1,
            JobResult::Failed { input, error } => {
                summary.failed += 1;
                summary.failure_details.push((input.clone(), error.clone()));
            }
        }
    }

    summary
}

pub fn print(summary: &Summary) {
    println!(
        "Summary: total={}, converted={}, skipped={}, failed={}, workers={}",
        summary.total, summary.converted, summary.skipped, summary.failed, summary.workers
    );

    for (input, error) in &summary.failure_details {
        eprintln!("FAILED {}: {}", input.display(), error);
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::convert::{ExecutionReport, JobResult};

    use super::from_report;

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

        let summary = from_report(&report);
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
