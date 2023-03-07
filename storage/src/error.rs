#[derive(thiserror::Error, Debug)]
pub enum StorageError {
    #[error("Serialization/deserialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    IOError(#[from] std::io::Error),

    #[error("S3 storage error: {0}")]
    S3Error(#[from] s3::error::S3Error),

    #[error("S3 storage failure code {0}")]
    S3Failure(u16),

    #[error("Camera with name \"{0}\" was not found")]
    NoSuchCamera(String),

    #[error(
        "Error in a storage workflow resulting in a subset of actions being successful (see logs)"
    )]
    WorkflowPartialError,

    #[error("A requested item was not found")]
    NotFound,
}

pub type StorageResult<T> = Result<T, StorageError>;
