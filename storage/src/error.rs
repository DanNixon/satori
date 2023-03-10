#[derive(thiserror::Error, Debug)]
pub enum StorageError {
    #[error("serde_json error: {0}")]
    SerdeJsonError(#[from] serde_json::Error),

    #[error("serde_cbor error: {0}")]
    SerdeCborError(#[from] serde_cbor::Error),

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

    #[error("A key that is required to perform an en/decrption operation is not provided")]
    KeyMissing,

    #[error("Encryption key length incorrect, expected {0}, got {1}")]
    KeyLengthError(usize, usize),

    #[error("PEM error")]
    PemError,

    #[error("HPKE error: {0}")]
    HpkeError(#[from] hpke::HpkeError),
}

pub type StorageResult<T> = Result<T, StorageError>;
