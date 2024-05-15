#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    #[error("Config file error: {0}")]
    ConfigFile(#[from] satori_common::ConfigFileError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Prometheus exporter error: {0}")]
    Prometheus(#[from] metrics_exporter_prometheus::BuildError),
}

pub(crate) type Result<T> = std::result::Result<T, Error>;
