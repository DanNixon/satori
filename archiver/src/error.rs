#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    #[error("Config file error: {0}")]
    ConfigFile(#[from] satori_common::ConfigFileError),

    #[error("Prometheus exporter error: {0}")]
    Prometheus(#[from] metrics_exporter_prometheus::BuildError),

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

pub(crate) type Result<T> = std::result::Result<T, Error>;
