#[derive(thiserror::Error, Debug)]
pub(crate) enum ArchiverError {
    #[error("Storage error: {0}")]
    StorageError(#[from] satori_storage::StorageError),

    #[error("Camera not found")]
    CameraNotFound,

    #[error("Network error: {0}")]
    NetworkError(#[from] reqwest::Error),

    #[error("IO error: {0}")]
    IOError(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("URL manipulation error")]
    UrlError,
}

pub(crate) type ArchiverResult<T> = Result<T, ArchiverError>;
