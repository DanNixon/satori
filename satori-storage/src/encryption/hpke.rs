use super::KeyOperations;
use crate::{StorageError, StorageResult};
use bytes::Bytes;
use generic_array::GenericArray;
use hpke::{Deserializable, Kem, Serializable};
use rand::{SeedableRng, rngs::StdRng};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error};
use zeroize::Zeroize;

type SelectedKem = hpke::kem::X25519HkdfSha256;
type SelectedAead = hpke::aead::ChaCha20Poly1305;
type SelectedKdf = hpke::kdf::HkdfSha384;

type PublicKey = <SelectedKem as hpke::Kem>::PublicKey;
type PrivateKey = <SelectedKem as hpke::Kem>::PrivateKey;
type EncappedKey = <SelectedKem as hpke::Kem>::EncappedKey;

#[derive(Clone)]
pub struct Hpke {
    public_key: PublicKey,
    private_key: Option<PrivateKey>,
}

impl std::fmt::Debug for Hpke {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "HPKE ({})",
            if self.private_key.is_some() {
                "pub+priv"
            } else {
                "pub"
            }
        )
    }
}

impl KeyOperations for Hpke {
    fn generate() -> Self {
        let mut rng = StdRng::from_os_rng();
        let (private_key, public_key) = SelectedKem::gen_keypair(&mut rng);
        Self {
            public_key,
            private_key: Some(private_key),
        }
    }

    fn encrypt(&self, id: Bytes, data: Bytes) -> StorageResult<Bytes> {
        let mut csprng = StdRng::from_os_rng();

        let (capped_key, ciphertext): (EncappedKey, Vec<u8>) =
            hpke::single_shot_seal::<SelectedAead, SelectedKdf, SelectedKem, _>(
                &hpke::OpModeS::Base,
                &self.public_key,
                b"satori".as_slice(),
                &data,
                &id,
                &mut csprng,
            )?;

        let payload = Payload {
            key: capped_key,
            ciphertext: ciphertext.into(),
        };

        let mut data: Vec<u8> = Vec::new();
        ciborium::into_writer(&payload, &mut data)?;

        Ok(data.into())
    }

    fn decrypt(&self, id: Bytes, data: Bytes) -> StorageResult<Bytes> {
        match &self.private_key {
            None => Err(StorageError::KeyMissing),
            Some(private_key) => {
                let payload: Payload = ciborium::from_reader(&*data)?;

                let data = hpke::single_shot_open::<SelectedAead, SelectedKdf, SelectedKem>(
                    &hpke::OpModeR::Base,
                    private_key,
                    &payload.key,
                    b"satori".as_slice(),
                    &payload.ciphertext,
                    &id,
                )?;

                Ok(data.into())
            }
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
struct Payload {
    #[serde(with = "serde_encapped_key")]
    key: EncappedKey,
    ciphertext: Bytes,
}

macro_rules! impl_serde {
    ($modname:ident, $t:ty) => {
        pub(crate) mod $modname {
            use super::*;

            pub(crate) fn serialize<S: Serializer>(
                val: &$t,
                serializer: S,
            ) -> Result<S::Ok, S::Error> {
                let mut arr = val.to_bytes();
                let ret = arr.serialize(serializer);
                arr.zeroize();
                ret
            }

            pub(crate) fn deserialize<'de, D: Deserializer<'de>>(
                deserializer: D,
            ) -> Result<$t, D::Error> {
                let mut arr = GenericArray::<u8, <$t as Serializable>::OutputSize>::deserialize(
                    deserializer,
                )?;
                let ret = <$t>::from_bytes(&arr).map_err(D::Error::custom);
                arr.zeroize();
                ret
            }
        }
    };
}

impl_serde!(serde_encapped_key, EncappedKey);

mod deserialize {
    use super::{Hpke, PrivateKey, PublicKey};
    use generic_array::GenericArray;
    use hpke::{Deserializable, Serializable};
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    #[derive(Debug, Clone, Deserialize, Serialize)]
    struct SerialisedRepr {
        public_key: GenericArray<u8, <PublicKey as Serializable>::OutputSize>,
        private_key: Option<GenericArray<u8, <PrivateKey as Serializable>::OutputSize>>,
    }

    impl<'de> Deserialize<'de> for Hpke {
        fn deserialize<D>(deserializer: D) -> Result<Hpke, D::Error>
        where
            D: Deserializer<'de>,
        {
            use serde::de::Error;

            let repr = SerialisedRepr::deserialize(deserializer)?;

            let pk = PublicKey::from_bytes(&repr.public_key).map_err(Error::custom)?;

            let sk = repr
                .private_key
                .as_ref()
                .map(|sk| PrivateKey::from_bytes(sk).map_err(Error::custom))
                .transpose()?;

            Ok(Hpke {
                public_key: pk,
                private_key: sk,
            })
        }
    }

    impl Serialize for Hpke {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            let public_key = self.public_key.to_bytes();
            let private_key = self.private_key.as_ref().map(|key| key.to_bytes());

            let repr = SerialisedRepr {
                public_key,
                private_key,
            };

            repr.serialize(serializer)
        }
    }

    #[cfg(test)]
    mod test {
        use crate::KeyOperations;

        use super::*;

        #[test]
        fn test_serialize_deserialize_round_trip() {
            let original: Hpke = Hpke::generate();

            let serialized = toml::to_string(&original).unwrap();

            let deserialized: Hpke = toml::from_str(&serialized).unwrap();

            assert_eq!(
                deserialized.public_key.to_bytes(),
                original.public_key.to_bytes()
            );
            assert_eq!(
                deserialized.private_key.unwrap().to_bytes(),
                original.private_key.unwrap().to_bytes(),
            );
        }

        #[test]
        fn test_serialize_deserialize_public_only() {
            let mut original = Hpke::generate();
            original.private_key = None;

            let serialized = toml::to_string(&original).unwrap();

            let deserialized: Hpke = toml::from_str(&serialized).unwrap();

            assert_eq!(
                deserialized.public_key.to_bytes(),
                original.public_key.to_bytes()
            );
            assert!(deserialized.private_key.is_none());
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::encryption::test::encryption_test;

    fn keypair() -> (Hpke, Hpke) {
        let sk = Hpke::generate();

        let mut pk = sk.clone();
        pk.private_key = None;

        (pk, sk)
    }

    encryption_test!(basic_round_trip, keypair);
    encryption_test!(cannot_decrypt_without_sk, keypair);

    fn mismatching_keypair() -> (Hpke, Hpke) {
        let sk = Hpke::generate();

        let mut pk = Hpke::generate();
        pk.private_key = None;

        (pk, sk)
    }

    encryption_test!(key_mismatch, mismatching_keypair);
}
