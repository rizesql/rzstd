use crate::MAGIC_NUM;

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error("Invalid magic number. Expected: {MAGIC_NUM}, got: {0}")]
    InvalidMagicNum(u32),

    #[error("Window size {0} is out of bounds")]
    WindowSizeOutOfBounds(u64),

    #[error("Reserved bit is set")]
    ReservedBitSet,

    #[error("Reserved block type")]
    ReservedBlock,

    #[error("Invalid block type {0}")]
    InvalidBlockType(u32),

    #[error("Block size {0} is out of bounds")]
    BlockSizeOutOfBounds(u32),

    #[error("Missing compressed size")]
    MissingCompressedSize,

    #[error("Missing Huffman size")]
    MissingHuffTable,

    #[error("Missing modes")]
    MissingModes,

    #[error("Missing sequence table")]
    MissingSeqTable,

    #[error("Missing block size")]
    MissingBlockSize,

    #[error("Corrupted data")]
    Corruption,

    #[error("Copied data size is out of bounds")]
    CopiedSizeOutOfBounds,

    #[error(transparent)]
    IO(#[from] rzstd_io::Error),

    #[error(transparent)]
    Huff0(#[from] rzstd_huff0::Error),

    #[error(transparent)]
    FSE(#[from] rzstd_fse::Error),
}

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Self::IO(rzstd_io::Error::IO(value))
    }
}
