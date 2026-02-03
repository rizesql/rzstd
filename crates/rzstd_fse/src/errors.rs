#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error(transparent)]
    IO(#[from] rzstd_io::Error),

    #[error("Invalid accuracy log")]
    InvalidAccuracyLog,

    #[error("FSE accuracy log mismatch")]
    AccuracyLogMismatch,

    #[error("Too many symbols")]
    TooManySymbols,

    #[error("FSE sum mismatch")]
    SumMismatch,

    #[error("Spread overflow")]
    SpreadOverflow,

    #[error("Fast-spread alignment error")]
    FastSpreadAlignmentError,

    #[error("Table overflow")]
    TableOverflow,

    #[error("Table underfilled")]
    TableUnderfilled,

    #[error("Invalid state")]
    InvalidState,

    #[error("Data corruption detected")]
    Corruption,
}
