mod aes256gcm;
pub use aes256gcm::Aes256GcmKey as EncryptionKey;

use crate::StorageResult;
use bytes::Bytes;

pub trait KeyOperations {
    fn generate() -> Self;
    fn encrypt(&self, data: Bytes) -> StorageResult<Bytes>;
    fn decrypt(&self, data: Bytes) -> StorageResult<Bytes>;
}
