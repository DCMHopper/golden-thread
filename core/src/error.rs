use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("invalid argument: {0}")]
    InvalidArgument(String),
    #[error("invalid passphrase: {0}")]
    InvalidPassphrase(String),
    #[error("not implemented: {0}")]
    NotImplemented(String),
    #[error("crypto error: {0}")]
    Crypto(String),
}
