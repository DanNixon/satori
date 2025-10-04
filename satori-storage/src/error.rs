#[derive(thiserror::Error, Debug)]
pub enum StorageError {
    #[error("serde_json error: {0}")]
    SerdeJsonError(#[from] serde_json::Error),

    #[error("ciborium serialisation error: {0}")]
    CiboriumSerialisationError(#[from] ciborium::ser::Error<std::io::Error>),

    #[error("ciborium deserialisation error: {0}")]
    CiboriumDeserialisationError(#[from] ciborium::de::Error<std::io::Error>),

    #[error("toml serialization error: {0}")]
    SerdeTomlSerError(#[from] toml::ser::Error),

    #[error("toml deserialization error: {0}")]
    SerdeTomlDeError(#[from] toml::de::Error),

    #[error("IO error: {0}")]
    IOError(#[from] std::io::Error),

    #[error("S3 storage error: {0}")]
    S3Error(#[from] s3::error::S3Error),

    #[error("S3 storage failure code {0}")]
    S3Failure(u16),

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
