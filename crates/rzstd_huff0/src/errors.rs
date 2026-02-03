#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error(transparent)]
    IO(#[from] rzstd_io::Error),

    #[error(transparent)]
    FSE(#[from] rzstd_fse::Error),

    #[error("Data corruption detected")]
    Corruption,

    #[error("Table overflow")]
    TableOverflow,
}
