mod cli;
mod config;
mod convert;
mod discover;
mod ffmpeg;
mod plan;
mod summary;

use anyhow::Result;

fn main() -> Result<()> {
    let cli = cli::parse();
    let config = config::resolve(cli)?;

    let inputs = discover::discover(&config)?;
    let jobs = plan::plan(&config, inputs)?;
    let report = convert::execute(&config, jobs);
    let summary = summary::Summary::from_report(&report);

    summary.print();

    if summary.failed > 0 {
        std::process::exit(1);
    }

    Ok(())
}
