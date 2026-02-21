use std::{
    fs::File,
    io::{BufReader, BufWriter, stdout},
    path::PathBuf,
};

use clap::{Args, Parser, Subcommand};
use miette::IntoDiagnostic;
use rzstd_decompress::MAX_BLOCK_SIZE;
use tracing_subscriber::{EnvFilter, prelude::*};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Decompresses a file
    Decompress(DecompressArgs),
}

#[derive(Args)]
struct DecompressArgs {
    /// Input file to decompress
    input: PathBuf,

    /// Output file
    output: Option<PathBuf>,
}

fn main() -> miette::Result<()> {
    let cli = Cli::parse();

    let file_appender = tracing_appender::rolling::never("target", "dump.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(non_blocking)
        .with_ansi(false)
        .without_time()
        .with_level(false);

    let stdout_layer = tracing_subscriber::fmt::layer()
        .with_writer(stdout)
        .with_ansi(true)
        .without_time()
        .with_level(false);

    tracing_subscriber::registry()
        .with(EnvFilter::from_default_env())
        .with(file_layer)
        .with(stdout_layer)
        .init();

    match cli.command {
        Commands::Decompress(args) => {
            let output_file = {
                let output = args.output.unwrap_or(
                    args.input.file_stem().expect("Unnamed input file").into(),
                );
                File::create(output).into_diagnostic()?
            };

            let input_file = File::open(args.input).into_diagnostic()?;
            let reader = BufReader::new(input_file);

            let mut writer = BufWriter::new(output_file);

            let window_size = 100 * 1024 * 1024;
            let mut window_buffer = vec![0u8; window_size + MAX_BLOCK_SIZE as usize];

            let mut decoder =
                rzstd_decompress::Decoder::new(reader, &mut window_buffer, window_size);

            decoder.decode(&mut writer).into_diagnostic()?;
        }
    }
    Ok(())
}
