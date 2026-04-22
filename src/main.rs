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
    let results = convert::execute(&config, jobs);
    let summary = summary::from_results(&results);

    summary::print(&summary);

    if summary.failed > 0 {
        std::process::exit(1);
    }

    Ok(())
}
