use std::{
    fs::File,
    io::{BufReader, BufWriter},
};

use miette::IntoDiagnostic;
use rzstd_decompress::MAX_BLOCK_SIZE;

fn main() -> miette::Result<()> {
    let input_file = File::open("test.zst").into_diagnostic()?;
    let reader = BufReader::new(input_file);

    let output_file = File::create("output.decompressed").into_diagnostic()?;
    let mut writer = BufWriter::new(output_file);

    let mut window_buffer = vec![0u8; 8 * 1024 * 1024 + MAX_BLOCK_SIZE as usize];

    let mut decoder = rzstd_decompress::Decoder::new(
        reader,
        &mut window_buffer,
        8 * 1024 * 1024 as usize,
    );

    println!("Starting decompression...");
    decoder.decode(&mut writer).into_diagnostic()?;
    Ok(())
}
