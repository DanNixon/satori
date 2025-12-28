mod hpke;

#[cfg(test)]
mod test;

use crate::StorageResult;
use bytes::Bytes;
use serde::Deserialize;

pub trait KeyOperations {
    fn generate() -> Self;
    fn encrypt(&self, id: Bytes, data: Bytes) -> StorageResult<Bytes>;
    fn decrypt(&self, id: Bytes, data: Bytes) -> StorageResult<Bytes>;
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EncryptionKey {
    Hpke(hpke::Hpke),
}

impl KeyOperations for EncryptionKey {
    fn generate() -> Self {
        // Default to generating a HPKE key (although this function is unlikely
        // to be called, it is just here to fulfill the trait requirements).
        Self::Hpke(hpke::Hpke::generate())
    }

    fn encrypt(&self, id: Bytes, data: Bytes) -> StorageResult<Bytes> {
        match &self {
            Self::Hpke(k) => k.encrypt(id, data),
        }
    }

    fn decrypt(&self, id: Bytes, data: Bytes) -> StorageResult<Bytes> {
        match &self {
            Self::Hpke(k) => k.decrypt(id, data),
        }
    }
}

pub(crate) mod info {
    use bytes::Bytes;

    pub(crate) fn event_info_from_filename(filename: &str) -> Bytes {
        filename.as_bytes().to_owned().into()
    }

    pub(crate) fn segment_info_from_camera_and_filename(
        camera_name: &str,
        filename: &str,
    ) -> Bytes {
        format!("{camera_name} {}", filename)
            .as_bytes()
            .to_owned()
            .into()
    }
}
