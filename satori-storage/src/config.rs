use crate::EncryptionConfig;
use serde::Deserialize;
use url::Url;

#[derive(Debug, Deserialize)]
pub struct StorageConfig {
    pub(crate) url: Url,

    #[serde(default)]
    pub(crate) encryption: EncryptionConfig,
}
