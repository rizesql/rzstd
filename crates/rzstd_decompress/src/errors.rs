use crate::MAGIC_NUM;

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error("Invalid magic number. Expected: {MAGIC_NUM:x}, got: {0:x}")]
    #[diagnostic(
        code(rzstd::decompress::invalid_magic_num),
        help("The input data does not start with the Zstandard magic number.")
    )]
    InvalidMagicNum(u32),

    #[error("Window size {0} is out of bounds")]
    #[diagnostic(
        code(rzstd::decompress::window_size_out_of_bounds),
        help("The requested window size is too large or invalid.")
    )]
    WindowSizeOutOfBounds(u64),

    #[error("Reserved bit is set")]
    #[diagnostic(
        code(rzstd::decompress::reserved_bit_set),
        help("A reserved bit in the frame header is set, which is not allowed.")
    )]
    ReservedBitSet,

    #[error("Reserved block type")]
    #[diagnostic(
        code(rzstd::decompress::reserved_block),
        help("Encountered a block type that is reserved.")
    )]
    ReservedBlock,

    #[error("Invalid block type {0}")]
    #[diagnostic(
        code(rzstd::decompress::invalid_block_type),
        help("The block type is not recognized.")
    )]
    InvalidBlockType(u32),

    #[error("Block size {0} is out of bounds")]
    #[diagnostic(
        code(rzstd::decompress::block_size_out_of_bounds),
        help("The block size exceeds the maximum allowed.")
    )]
    BlockSizeOutOfBounds(u32),

    #[error("Missing compressed size")]
    #[diagnostic(
        code(rzstd::decompress::missing_compressed_size),
        help("Compressed block requires a compressed size.")
    )]
    MissingCompressedSize,

    #[error("Missing Huffman size")]
    #[diagnostic(
        code(rzstd::decompress::missing_huffman_table),
        help("Compressed literals block missing Huffman tree description.")
    )]
    MissingHuffTable,

    #[error("Missing modes")]
    #[diagnostic(
        code(rzstd::decompress::missing_modes),
        help("Sequences section header missing compression modes.")
    )]
    MissingModes,

    #[error("Missing sequence table")]
    #[diagnostic(
        code(rzstd::decompress::missing_seq_table),
        help("A sequence table is required but missing.")
    )]
    MissingSeqTable,

    #[error("Missing block size")]
    #[diagnostic(
        code(rzstd::decompress::missing_block_size),
        help("The block header is incomplete.")
    )]
    MissingBlockSize,

    #[error("Literals size {0} exceeds max block size")]
    #[diagnostic(
        code(rzstd::decompress::literals_size_too_large),
        help(
            "The regenerated size of the literals section exceeds the maximum allowed block size."
        )
    )]
    LiteralsSizeTooLarge(u32),

    #[error("Compressed size {0} exceeds max block size")]
    #[diagnostic(
        code(rzstd::decompress::compressed_size_too_large),
        help(
            "The compressed size of the literals section exceeds the maximum allowed block size."
        )
    )]
    CompressedSizeTooLarge(u32),

    #[error("Extra bits remaining in stream: {0}")]
    #[diagnostic(
        code(rzstd::decompress::extra_bits),
        help("The stream should be fully consumed, but bits remain.")
    )]
    ExtraBitsInStream(usize),

    #[error("Jump table error: {0}")]
    #[diagnostic(
        code(rzstd::decompress::jump_table_error),
        help("Error parsing the 4-stream jump table in the literals section.")
    )]
    JumpTableError(String),

    #[error("Literals buffer too small")]
    #[diagnostic(
        code(rzstd::decompress::literals_buffer_too_small),
        help("The output buffer for literals is too small for the decoded data.")
    )]
    LiteralsBufferTooSmall,

    #[error("Missing table for repeat mode")]
    #[diagnostic(
        code(rzstd::decompress::missing_table_repeat),
        help("A repeat mode was specified but no previous table exists to repeat.")
    )]
    MissingTableForRepeat,

    #[error("Empty RLE source")]
    #[diagnostic(
        code(rzstd::decompress::empty_rle_source),
        help("RLE mode specified but source data is empty.")
    )]
    EmptyRLESource,

    #[error("Invalid FSE code: {0}")]
    #[diagnostic(
        code(rzstd::decompress::invalid_fse_code),
        help("Decoded FSE code is invalid or out of bounds for the symbol type.")
    )]
    InvalidFSECode(u8),

    #[error("Literals buffer overread: idx {idx}, len {len}")]
    #[diagnostic(
        code(rzstd::decompress::literals_buffer_overread),
        help(
            "Attempted to read past the end of the literals buffer during sequence execution."
        )
    )]
    LiteralsBufferOverread { idx: usize, len: usize },

    #[error("Invalid offset code: {0}")]
    #[diagnostic(
        code(rzstd::decompress::invalid_offset_code),
        help("The offset code is invalid (e.g., calculation resulted in underflow).")
    )]
    InvalidOffsetCode(u32),

    #[error("Zero offset detected")]
    #[diagnostic(
        code(rzstd::decompress::zero_offset),
        help("An offset of zero is invalid in Zstandard.")
    )]
    ZeroOffset,

    #[error("Corrupted data")]
    #[diagnostic(
        code(rzstd::decompress::corruption),
        help("Generic data corruption detected.")
    )]
    Corruption,

    #[error("Copied data size is out of bounds")]
    #[diagnostic(
        code(rzstd::decompress::copied_size_out_of_bounds),
        help("Attempted to copy more data than allowed.")
    )]
    CopiedSizeOutOfBounds,

    #[error(transparent)]
    #[diagnostic(code(rzstd::decompress::io))]
    IO(#[from] rzstd_io::Error),

    #[error(transparent)]
    #[diagnostic(code(rzstd::decompress::huff0))]
    Huff0(#[from] rzstd_huff0::Error),

    #[error(transparent)]
    #[diagnostic(code(rzstd::decompress::fse))]
    FSE(#[from] rzstd_fse::Error),
}

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Self::IO(rzstd_io::Error::IO(value))
    }
}
