use satori_storage::StorageConfig;
use serde::Deserialize;
use std::net::SocketAddr;

#[derive(Debug, Deserialize)]
pub(crate) struct Config {
    pub(crate) http_server_address: SocketAddr,

    pub(crate) storage: StorageConfig,
}
