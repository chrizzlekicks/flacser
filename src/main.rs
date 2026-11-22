use anyhow::Result;
use flacser::{cli, config, doctor, interrupt, summary};

fn main() -> Result<()> {
    let cli = cli::parse();

    match cli.command {
        cli::Commands::Convert(convert) => run_convert(convert),
        cli::Commands::Doctor(doctor) => run_doctor(doctor),
    }
}

fn run_convert(convert: cli::ConvertArgs) -> Result<()> {
    let interrupt = interrupt::InterruptFlag::new();
    interrupt::install_handler(interrupt.clone())?;

    let config = config::Config::from_convert_args(convert)?;

    let reporter = std::sync::OnceLock::new();
    let report = flacser::run_conversion_with_presentation(
        &config,
        &interrupt,
        &|jobs| {
            reporter
                .set(flacser::progress::ProgressReporter::new(jobs.len()))
                .unwrap();
            if jobs
                .iter()
                .any(|job| job.target_format == flacser::audio_format::AudioFormat::Wav)
            {
                eprintln!("{}", flacser::ffmpeg::WAV_METADATA_NOTE);
            }
        },
        &|| reporter.get().unwrap().finish_job(),
    )?;
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
