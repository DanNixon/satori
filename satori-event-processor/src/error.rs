#[derive(thiserror::Error, Debug)]
pub(crate) enum EventProcessorError {
    #[error("Camera with name \"{0}\" was not found")]
    NoSuchCamera(String),

    #[error("IO error: {0}")]
    IOError(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("Playlist parse error")]
    PlaylistParseError,

    #[error("Network error: {0}")]
    NetworkError(#[from] reqwest::Error),
}

pub(crate) type EventProcessorResult<T> = Result<T, EventProcessorError>;
