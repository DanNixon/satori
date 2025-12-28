use crate::EncryptionKey;
use serde::Deserialize;
use url::Url;

#[derive(Debug, Deserialize)]
pub struct StorageConfig {
    pub(crate) url: Url,
    pub(crate) encryption: EncryptionKey,
}
