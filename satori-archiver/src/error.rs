#[derive(thiserror::Error, Debug)]
pub(crate) enum ArchiverError {
    #[error("Storage error: {0}")]
    Storage(#[from] satori_storage::StorageError),

    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("IO error: {0}")]
    IO(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("URL manipulation error")]
    Url,
}

pub(crate) type ArchiverResult<T> = Result<T, ArchiverError>;
