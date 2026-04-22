use std::path::PathBuf;

use crate::convert::JobResult;

pub struct Summary {
    pub total: usize,
    pub converted: usize,
    pub skipped: usize,
    pub failed: usize,
    pub failure_details: Vec<(PathBuf, String)>,
}

pub fn from_results(results: &[JobResult]) -> Summary {
    let mut summary = Summary {
        total: results.len(),
        converted: 0,
        skipped: 0,
        failed: 0,
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
        "Summary: total={}, converted={}, skipped={}, failed={}",
        summary.total, summary.converted, summary.skipped, summary.failed
    );

    for (input, error) in &summary.failure_details {
        eprintln!("FAILED {}: {}", input.display(), error);
    }
}
