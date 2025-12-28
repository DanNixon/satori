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
    use crate::{StorageError, StorageResult};
    use hpke::Deserializable;
    use serde::{Deserialize, Deserializer};

    #[derive(Debug, Clone, Deserialize)]
    struct SerialisedRepr {
        public_key: String,
        private_key: Option<String>,
    }

    fn parse_pem_x25519_key(s: &str) -> StorageResult<Vec<u8>> {
        let bytes = pem_rfc7468::decode_vec(s.as_bytes())
            .map_err(|_| StorageError::PemError)?
            .1;

        if bytes.len() < 32 {
            Err(StorageError::KeyLengthError(32, bytes.len()))
        } else {
            Ok(bytes[bytes.len() - 32..].to_owned())
        }
    }

    impl<'de> Deserialize<'de> for Hpke {
        fn deserialize<D>(deserializer: D) -> Result<Hpke, D::Error>
        where
            D: Deserializer<'de>,
        {
            use serde::de::Error;

            let repr = SerialisedRepr::deserialize(deserializer)?;

            let pk = PublicKey::from_bytes(
                &parse_pem_x25519_key(&repr.public_key).map_err(Error::custom)?,
            )
            .map_err(Error::custom)?;

            let sk = repr
                .private_key
                .map(|sk| -> Result<_, D::Error> {
                    PrivateKey::from_bytes(&parse_pem_x25519_key(&sk).map_err(Error::custom)?)
                        .map_err(Error::custom)
                })
                .and_then(|sk| sk.ok());

            Ok(Hpke {
                public_key: pk,
                private_key: sk,
            })
        }
    }

    #[cfg(test)]
    mod test {
        use super::*;

        #[test]
        fn test_parse_pem_x25519_key() {
            let pem = "
-----BEGIN PUBLIC KEY-----
E3HEpQ4ck1CRXCXHoDg6m5meXJ0I0fpfTy3NXIKC+Vg=
-----END PUBLIC KEY-----
";

            let key = parse_pem_x25519_key(pem).unwrap();

            assert_eq!(
                key,
                hex::decode("1371c4a50e1c9350915c25c7a0383a9b999e5c9d08d1fa5f4f2dcd5c8282f958")
                    .unwrap()
            );
        }

        #[test]
        fn test_parse_pem_x25519_key_bad_base64() {
            let pem = "
-----BEGIN PUBLIC KEY-----
E3HEpQ4ck1CRXCXHoDg6m5meXJ0I0fpfTy3NXIKC+Vg
-----END PUBLIC KEY-----
";

            let result = parse_pem_x25519_key(pem);

            assert!(result.is_err());
            assert_eq!(result.unwrap_err().to_string(), "PEM error");
        }

        #[test]
        fn test_parse_pem_x25519_key_too_short() {
            let pem = "
-----BEGIN PUBLIC KEY-----
dGVzdAo=
-----END PUBLIC KEY-----
";

            let result = parse_pem_x25519_key(pem);

            assert!(result.is_err());
            assert_eq!(
                result.unwrap_err().to_string(),
                "Encryption key length incorrect, expected 32, got 5"
            );
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::encryption::test::encryption_test;
    use hpke::Serializable;

    #[test]
    fn deserialize_public_and_private_key_only() {
        let repr = "
public_key = \"\"\"
-----BEGIN PUBLIC KEY-----
E3HEpQ4ck1CRXCXHoDg6m5meXJ0I0fpfTy3NXIKC+Vg=
-----END PUBLIC KEY-----
\"\"\"
private_key = \"\"\"
-----BEGIN PRIVATE KEY-----
0Kre7tGZ1d9L1gfDyL3CayRXKt5RtcIetEDjzqCVb0s=
-----END PRIVATE KEY-----
\"\"\"
        ";

        let keys: Hpke = toml::from_str(repr).unwrap();

        let pk_bytes = keys.public_key.to_bytes();
        assert_eq!(pk_bytes.len(), 32);
        assert_eq!(
            pk_bytes.as_slice(),
            hex::decode("1371c4a50e1c9350915c25c7a0383a9b999e5c9d08d1fa5f4f2dcd5c8282f958")
                .unwrap()
        );

        let sk_bytes = keys.private_key.unwrap().to_bytes();
        assert_eq!(sk_bytes.len(), 32);
        assert_eq!(
            sk_bytes.as_slice(),
            hex::decode("d0aadeeed199d5df4bd607c3c8bdc26b24572ade51b5c21eb440e3cea0956f4b")
                .unwrap()
        );
    }

    #[test]
    fn deserialize_public_and_private_openssl() {
        // openssl genpkey -algorithm X25519 -out x25519_sk.pem
        // openssl pkey -in x25519_sk.pem -pubout
        let repr = "
public_key = \"\"\"
-----BEGIN PUBLIC KEY-----
MCowBQYDK2VuAyEAZWyBUeaFatX3a3/OnqFljoEhAUHjrLgDJzzc5EqR/ho=
-----END PUBLIC KEY-----
\"\"\"
private_key = \"\"\"
-----BEGIN PRIVATE KEY-----
MC4CAQAwBQYDK2VuBCIEIPAn/aQduWFV5VAlGQF79sBuzQItqFWu6FdJ4B77/UJ7
-----END PRIVATE KEY-----
\"\"\"
        ";

        let keys: Hpke = toml::from_str(repr).unwrap();

        // openssl pkey -in x25519_sk.pem -text
        let pk_bytes = keys.public_key.to_bytes();
        assert_eq!(pk_bytes.len(), 32);
        assert_eq!(
            pk_bytes.as_slice(),
            hex::decode("656c8151e6856ad5f76b7fce9ea1658e81210141e3acb803273cdce44a91fe1a")
                .unwrap()
        );

        // openssl pkey -in x25519_sk.pem -text
        let sk_bytes = keys.private_key.unwrap().to_bytes();
        assert_eq!(sk_bytes.len(), 32);
        assert_eq!(
            sk_bytes.as_slice(),
            hex::decode("f027fda41db96155e5502519017bf6c06ecd022da855aee85749e01efbfd427b")
                .unwrap()
        );
    }

    #[test]
    fn deserialize_public_only_openssl() {
        // openssl pkey -in x25519_sk.pem -pubout
        let repr = "
public_key = \"\"\"
-----BEGIN PUBLIC KEY-----
MCowBQYDK2VuAyEAZWyBUeaFatX3a3/OnqFljoEhAUHjrLgDJzzc5EqR/ho=
-----END PUBLIC KEY-----
\"\"\"
        ";

        let keys: Hpke = toml::from_str(repr).unwrap();

        // openssl pkey -in x25519_sk.pem -text
        let pk_bytes = keys.public_key.to_bytes();
        assert_eq!(pk_bytes.len(), 32);
        assert_eq!(
            pk_bytes.as_slice(),
            hex::decode("656c8151e6856ad5f76b7fce9ea1658e81210141e3acb803273cdce44a91fe1a")
                .unwrap()
        );

        assert!(keys.private_key.is_none());
    }

    fn keypair() -> (Hpke, Hpke) {
        let pk = "
public_key = \"\"\"
-----BEGIN PUBLIC KEY-----
MCowBQYDK2VuAyEAZWyBUeaFatX3a3/OnqFljoEhAUHjrLgDJzzc5EqR/ho=
-----END PUBLIC KEY-----
\"\"\"
        ";
        let pk: Hpke = toml::from_str(pk).unwrap();

        let sk = "
public_key = \"\"\"
-----BEGIN PUBLIC KEY-----
MCowBQYDK2VuAyEAZWyBUeaFatX3a3/OnqFljoEhAUHjrLgDJzzc5EqR/ho=
-----END PUBLIC KEY-----
\"\"\"
private_key = \"\"\"
-----BEGIN PRIVATE KEY-----
MC4CAQAwBQYDK2VuBCIEIPAn/aQduWFV5VAlGQF79sBuzQItqFWu6FdJ4B77/UJ7
-----END PRIVATE KEY-----
\"\"\"
        ";
        let sk: Hpke = toml::from_str(sk).unwrap();

        (pk, sk)
    }

    encryption_test!(basic_round_trip, keypair);
    encryption_test!(cannot_decrypt_without_sk, keypair);

    fn mismatching_keypair() -> (Hpke, Hpke) {
        let pk = "
public_key = \"\"\"
-----BEGIN PUBLIC KEY-----
MCowBQYDK2VuAyEA4xQouJZhiNpBedFJBs3lE8FIOMQtnMzZG426m2nVjko=
-----END PUBLIC KEY-----
\"\"\"
        ";
        let pk: Hpke = toml::from_str(pk).unwrap();

        let sk = "
public_key = \"\"\"
-----BEGIN PUBLIC KEY-----
MCowBQYDK2VuAyEAZWyBUeaFatX3a3/OnqFljoEhAUHjrLgDJzzc5EqR/ho=
-----END PUBLIC KEY-----
\"\"\"
private_key = \"\"\"
-----BEGIN PRIVATE KEY-----
MC4CAQAwBQYDK2VuBCIEIPAn/aQduWFV5VAlGQF79sBuzQItqFWu6FdJ4B77/UJ7
-----END PRIVATE KEY-----
\"\"\"
        ";
        let sk: Hpke = toml::from_str(sk).unwrap();

        (pk, sk)
    }

    encryption_test!(key_mismatch, mismatching_keypair);
}
