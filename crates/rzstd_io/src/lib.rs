mod bit_reader;
mod bit_writer;
mod reader;
mod reverse_bit_reader;

pub use bit_reader::BitReader;
pub use reader::Reader;
pub use reverse_bit_reader::ReverseBitReader;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Stream is empty")]
    EmptyStream,

    #[error("Stream end sentinel is missing")]
    MissingSentinel,

    #[error("Not enough bits in stream")]
    NotEnoughBits,

    #[error("Copy operation out of bounds")]
    CopyOutOfBounds,

    #[error(transparent)]
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
