#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error(transparent)]
    #[diagnostic(code(rzstd::fse::io))]
    IO(#[from] rzstd_io::Error),

    #[error("Invalid accuracy log: {0}")]
    #[diagnostic(
        code(rzstd::fse::invalid_accuracy_log),
        help("The accuracy log must be within valid bounds.")
    )]
    InvalidAccuracyLog(u8),

    #[error("FSE accuracy log mismatch. Expected <= {0}, got {1}")]
    #[diagnostic(
        code(rzstd::fse::accuracy_log_mismatch),
        help("The decoded accuracy log exceeds the table's maximum capability.")
    )]
    AccuracyLogMismatch(u8, u8),

    #[error("Too many symbols")]
    #[diagnostic(
        code(rzstd::fse::too_many_symbols),
        help("The number of symbols exceeds the maximum allowed.")
    )]
    TooManySymbols,

    #[error("FSE sum mismatch. Expected 0 remaining, got {0}")]
    #[diagnostic(
        code(rzstd::fse::sum_mismatch),
        help("The sum of probabilities does not match the expected power of 2.")
    )]
    SumMismatch(i32),

    #[error("Spread overflow")]
    #[diagnostic(
        code(rzstd::fse::spread_overflow),
        help("The spread of symbols overflowed the table size.")
    )]
    SpreadOverflow,

    #[error("Fast-spread alignment error (pos: {0})")]
    #[diagnostic(
        code(rzstd::fse::fast_spread_alignment),
        help(
            "Alignment error during fast spread table construction. This usually implies the table size and steps are not coprime or distribution is invalid."
        )
    )]
    FastSpreadAlignmentError(usize),

    #[error("Table overflow")]
    #[diagnostic(
        code(rzstd::fse::table_overflow),
        help(
            "The FSE table size exceeded the limit during low probability symbol spreading."
        )
    )]
    TableOverflow,

    #[error("Table underfilled")]
    #[diagnostic(
        code(rzstd::fse::table_underfilled),
        help("The FSE table was not completely filled.")
    )]
    TableUnderfilled,

    #[error("Invalid state")]
    #[diagnostic(
        code(rzstd::fse::invalid_state),
        help("The FSE state is invalid or out of bounds (state was 0).")
    )]
    InvalidState,

    #[error("Data corruption detected")]
    #[diagnostic(
        code(rzstd::fse::corruption),
        help("The FSE encoded data appears to be corrupted.")
    )]
    Corruption,
}
