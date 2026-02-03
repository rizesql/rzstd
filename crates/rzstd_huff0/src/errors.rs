#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error(transparent)]
    #[diagnostic(code(rzstd::huff0::io))]
    IO(#[from] rzstd_io::Error),

    #[error(transparent)]
    #[diagnostic(code(rzstd::huff0::fse))]
    FSE(#[from] rzstd_fse::Error),

    #[error("Data corruption detected")]
    #[diagnostic(
        code(rzstd::huff0::corruption),
        help("The Huff0 encoded data appears to be corrupted.")
    )]
    Corruption,

    #[error("Table overflow")]
    #[diagnostic(
        code(rzstd::huff0::table_overflow),
        help("The Huffman table overflowed.")
    )]
    TableOverflow,

    #[error("Weight {0} exceeds maximum bits {1}")]
    #[diagnostic(
        code(rzstd::huff0::weight_too_large),
        help(
            "A weight in the Huffman tree description exceeds the maximum allowed bits."
        )
    )]
    WeightTooLarge(u8, u8),

    #[error("Sum of weights is zero")]
    #[diagnostic(
        code(rzstd::huff0::zero_weight_sum),
        help(
            "The Huffman tree description is invalid because the sum of weights is zero."
        )
    )]
    ZeroWeightSum,

    #[error("Table log {0} exceeds maximum bits {1}")]
    #[diagnostic(
        code(rzstd::huff0::table_log_too_large),
        help("The calculated table depth exceeds the maximum allowed bits.")
    )]
    TableLogTooLarge(u8, u8),

    #[error("Invalid inferred weight (remainder: {0})")]
    #[diagnostic(
        code(rzstd::huff0::invalid_inferred_weight),
        help("The remaining weight for the last symbol is not a power of two.")
    )]
    InvalidInferredWeight(u32),

    #[error("Decoding table entry overwrite at index {0}")]
    #[diagnostic(
        code(rzstd::huff0::entry_overwrite),
        help(
            "Attempted to overwrite an existing entry in the decoding table. This indicates a corrupted tree description."
        )
    )]
    EntryOverwrite(usize),
}
