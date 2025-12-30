use crate::EncryptionKey;
use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Debug, Serialize, Deserialize)]
pub struct StorageConfig {
    pub(crate) url: Url,
    pub(crate) encryption: EncryptionKey,
}
