mod cli;
mod config;
mod convert;
mod discover;
mod ffmpeg;
mod plan;
mod progress;
mod sigint;
mod summary;

use anyhow::Result;

fn main() -> Result<()> {
    let sigint = sigint::SigintFlag::new();
    sigint::install_handler(sigint.shared())?;

    let cli = cli::parse();
    let config = config::resolve(cli)?;

    let inputs = discover::discover(&config)?;
    let jobs = plan::plan(&config, inputs)?;

    if ffmpeg::is_needed(&config, &jobs) {
        ffmpeg::check_availability()?;
    }

    let report = convert::execute(&config, jobs, &sigint);
    let summary = summary::Summary::from_report(&report);

    summary.print();

    if summary.interrupted > 0 || report.interrupted {
        std::process::exit(130);
    }

    if summary.failed > 0 {
        std::process::exit(1);
    }

    Ok(())
}
