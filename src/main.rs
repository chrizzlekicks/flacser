mod cli;
mod config;
mod convert;
mod discover;
mod doctor;
mod ffmpeg;
mod plan;
mod progress;
mod sigint;
mod summary;

use anyhow::Result;

fn main() -> Result<()> {
    let cli = cli::parse();

    match cli.command {
        cli::Commands::Convert(convert) => run_convert(convert),
        cli::Commands::Doctor(doctor) => run_doctor(doctor),
    }
}

fn run_convert(convert: cli::ConvertArgs) -> Result<()> {
    let sigint = sigint::SigintFlag::new();
    sigint::install_handler(sigint.shared())?;

    let config = config::Config::from_convert_args(convert);

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

fn run_doctor(doctor: cli::DoctorArgs) -> Result<()> {
    let report = doctor::run(doctor);
    report.print();

    if !report.is_ready() {
        std::process::exit(1);
    }

    Ok(())
}
