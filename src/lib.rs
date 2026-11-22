pub mod audio_format;
pub mod cli;
pub mod config;
pub mod convert;
pub mod discover;
pub mod doctor;
pub mod ffmpeg;
pub mod interrupt;
pub mod plan;
pub mod progress;
pub mod summary;

use anyhow::Result;
use config::Config;
use convert::ExecutionReport;
use interrupt::InterruptFlag;

/// Discover, plan, check dependencies, and execute a conversion.
pub fn run_conversion(config: &Config, interrupt: &InterruptFlag) -> Result<ExecutionReport> {
    run_conversion_inner(config, interrupt, &|_| {}, &|| {}, false)
}

/// Run the pipeline with callbacks for planned jobs and completed jobs.
pub fn run_conversion_with_presentation(
    config: &Config,
    interrupt: &InterruptFlag,
    planned: &dyn Fn(&[plan::ConversionJob]),
    progress: &(dyn Fn() + Sync),
) -> Result<ExecutionReport> {
    run_conversion_inner(config, interrupt, planned, progress, true)
}

fn run_conversion_inner(
    config: &Config,
    interrupt: &InterruptFlag,
    planned: &dyn Fn(&[plan::ConversionJob]),
    progress: &(dyn Fn() + Sync),
    streaming: bool,
) -> Result<ExecutionReport> {
    let inputs = discover::discover(config)?;
    let jobs = plan::plan(config, inputs)?;

    if ffmpeg::is_needed(config, &jobs) {
        ffmpeg::check_availability()?;
    }
    if ffmpeg::probe_is_needed(config, &jobs) {
        let probe = ffmpeg::probe_ffprobe_version();
        if !probe.executable_found {
            anyhow::bail!(ffmpeg::FFPROBE_NOT_FOUND);
        }
    }

    planned(&jobs);
    Ok(if streaming {
        convert::execute_streaming_with_progress(config, jobs, interrupt, progress)
    } else {
        convert::execute_with_progress(config, jobs, interrupt, progress)
    })
}
