use super::KeyOperations;
use crate::{StorageError, StorageResult};
use aes_gcm::{
    AeadCore, Aes256Gcm, Key, KeyInit, KeySizeUser, Nonce,
    aead::{Aead, common::Generate},
    aes::Aes256,
};
use bytes::Bytes;
use hybrid_array::Array;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Aes256GcmKey(Array<u8, <Aes256 as KeySizeUser>::KeySize>);

impl Aes256GcmKey {
    pub fn from_bytes(bytes: &[u8]) -> StorageResult<Self> {
        if bytes.len() == <Aes256 as KeySizeUser>::key_size() {
            let mut key_array = Array::<u8, <Aes256 as KeySizeUser>::KeySize>::default();
            key_array.copy_from_slice(bytes);
            Ok(Self(key_array))
        } else {
            Err(StorageError::Encryption)
        }
    }

    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_slice()
    }
}

impl KeyOperations for Aes256GcmKey {
    fn generate() -> Self {
        Self(Key::<Aes256>::generate_from_rng(&mut rand::rng()))
    }

    fn encrypt(&self, data: Bytes) -> StorageResult<Bytes> {
        let cipher = Aes256Gcm::new(&self.0);
        let nonce =
            Nonce::<<Aes256Gcm as AeadCore>::NonceSize>::generate_from_rng(&mut rand::rng());

        let ciphertext = cipher
            .encrypt(&nonce, data.iter().as_slice())
            .map_err(|_| StorageError::Encryption)?;

        let message = Message { nonce, ciphertext };
        let mut result = Vec::new();
        ciborium::ser::into_writer(&message, &mut result).map_err(|_| StorageError::Encryption)?;

        Ok(result.into())
    }

    fn decrypt(&self, data: Bytes) -> StorageResult<Bytes> {
        let message: Message =
            ciborium::de::from_reader(data.as_ref()).map_err(|_| StorageError::Encryption)?;

        let plaintext = Aes256Gcm::new(&self.0)
            .decrypt(&message.nonce, message.ciphertext.as_ref())
            .map_err(|_| StorageError::Encryption)?;

        Ok(plaintext.into())
    }
}

#[derive(Serialize, Deserialize)]
struct Message {
    nonce: Nonce<<Aes256Gcm as AeadCore>::NonceSize>,
    ciphertext: Vec<u8>,
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn basic_round_trip() {
        let key = Aes256GcmKey::generate();
        let plaintext = Bytes::from("hello world");
        let ciphertext = key.encrypt(plaintext.clone()).unwrap();
        let recovered_plaintext = key.decrypt(ciphertext).unwrap();
        assert_eq!(plaintext, recovered_plaintext);
    }
}
