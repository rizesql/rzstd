mod bit_reader;
mod reader;
mod reverse_bit_reader;

pub use bit_reader::BitReader;
pub use reader::*;
pub use reverse_bit_reader::ReverseBitReader;

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error("Stream is empty")]
    #[diagnostic(
        code(rzstd::io::empty_stream),
        help("The input stream ended unexpectedly. Verify the input data is complete.")
    )]
    EmptyStream,

    #[error("Stream end sentinel is missing")]
    #[diagnostic(
        code(rzstd::io::missing_sentinel),
        help("The stream should end with a sentinel bit/byte but it was not found.")
    )]
    MissingSentinel,

    #[error(
        "Not enough bits in stream. Requested: {requested:?}, Remaining: {remaining:?}"
    )]
    #[diagnostic(
        code(rzstd::io::not_enough_bits),
        help("Attempted to read more bits than are available in the stream.")
    )]
    NotEnoughBits { requested: usize, remaining: usize },

    #[error(transparent)]
    #[diagnostic(code(rzstd::io::io_error))]
    IO(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    pub(crate) fn pack_bits(chunk: &[bool]) -> u64 {
        chunk
            .iter()
            .enumerate()
            .map(|(i, &b)| (b as u64) << i)
            .fold(0, |acc, it| acc | it)
    }
}
