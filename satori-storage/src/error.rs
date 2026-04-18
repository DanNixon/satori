#[derive(thiserror::Error, Debug)]
pub enum StorageError {
    #[error("serde_json error: {0}")]
    SerdeJsonError(#[from] serde_json::Error),

    #[error("toml serialization error: {0}")]
    SerdeTomlSerError(#[from] toml::ser::Error),

    #[error("toml deserialization error: {0}")]
    SerdeTomlDeError(#[from] toml::de::Error),

    #[error("IO error: {0}")]
    IOError(#[from] std::io::Error),

    #[error("Object store error: {0}")]
    ObjectStoreError(#[from] object_store::Error),

    #[error("No appropriate backend for URL: {0}")]
    NoBackendForUrl(url::Url),

    #[error("Camera with name \"{0}\" was not found")]
    NoSuchCamera(String),

    #[error("A camera was not specified, but is required to be")]
    CameraMustBeSpecified,

    #[error(
        "Error in a storage workflow resulting in a subset of actions being successful (see logs)"
    )]
    WorkflowPartialError,

    #[error("A requested item was not found")]
    NotFound,

    #[error("Encryption/decryption failed")]
    Encryption,
}

pub type StorageResult<T> = Result<T, StorageError>;
