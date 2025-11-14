use clap::Parser;
use hpke::{Kem, Serializable};
use rand::{SeedableRng, rngs::StdRng};

type SelectedKem = hpke::kem::X25519HkdfSha256;
type PublicKey = <SelectedKem as hpke::Kem>::PublicKey;
type PrivateKey = <SelectedKem as hpke::Kem>::PrivateKey;

/// Generate a keypair for HPKE encrypted storage providers.
#[derive(Debug, Clone, Parser)]
pub(crate) struct GenerateKeyCommand {}

impl GenerateKeyCommand {
    pub(super) async fn execute(&self) -> miette::Result<()> {
        let mut csprng = StdRng::from_entropy();
        let (private_key, public_key): (PrivateKey, PublicKey) =
            SelectedKem::gen_keypair(&mut csprng);

        let pk_bytes = public_key.to_bytes();
        let sk_bytes = private_key.to_bytes();

        // Encode keys in PEM format compatible with OpenSSL
        let public_key_pem = encode_x25519_public_key(&pk_bytes)
            .map_err(|e| miette::miette!("Failed to encode public key: {}", e))?;
        let private_key_pem = encode_x25519_private_key(&sk_bytes)
            .map_err(|e| miette::miette!("Failed to encode private key: {}", e))?;

        println!("# HPKE Encryption Keypair");
        println!("#");
        println!("# Use this configuration in your storage provider config:");
        println!();
        println!("kind = \"hpke\"");
        println!("public_key = \"\"\"");
        println!("{}", public_key_pem.trim());
        println!("\"\"\"");
        println!("private_key = \"\"\"");
        println!("{}", private_key_pem.trim());
        println!("\"\"\"");

        Ok(())
    }
}

fn encode_x25519_public_key(key_bytes: &[u8]) -> Result<String, pem_rfc7468::Error> {
    // X25519 public keys in OpenSSL format have a 12-byte header:
    // 30 2a (SEQUENCE, 42 bytes)
    //   30 05 (SEQUENCE, 5 bytes)
    //     06 03 2b 65 6e (OID 1.3.101.110 = X25519)
    //   03 21 (BIT STRING, 33 bytes)
    //     00 (no unused bits)
    //     [32 bytes of key data]
    let mut der = Vec::with_capacity(12 + key_bytes.len());
    der.extend_from_slice(&[
        0x30, 0x2a, // SEQUENCE, 42 bytes
        0x30, 0x05, // SEQUENCE, 5 bytes
        0x06, 0x03, 0x2b, 0x65, 0x6e, // OID 1.3.101.110
        0x03, 0x21, // BIT STRING, 33 bytes
        0x00, // no unused bits
    ]);
    der.extend_from_slice(key_bytes);

    pem_rfc7468::encode_string("PUBLIC KEY", pem_rfc7468::LineEnding::default(), &der)
}

fn encode_x25519_private_key(key_bytes: &[u8]) -> Result<String, pem_rfc7468::Error> {
    // X25519 private keys in OpenSSL format have a 16-byte header:
    // 30 2e (SEQUENCE, 46 bytes)
    //   02 01 00 (INTEGER, version 0)
    //   30 05 (SEQUENCE, 5 bytes)
    //     06 03 2b 65 6e (OID 1.3.101.110 = X25519)
    //   04 22 (OCTET STRING, 34 bytes)
    //     04 20 (OCTET STRING, 32 bytes)
    //       [32 bytes of key data]
    let mut der = Vec::with_capacity(16 + key_bytes.len());
    der.extend_from_slice(&[
        0x30, 0x2e, // SEQUENCE, 46 bytes
        0x02, 0x01, 0x00, // INTEGER, version 0
        0x30, 0x05, // SEQUENCE, 5 bytes
        0x06, 0x03, 0x2b, 0x65, 0x6e, // OID 1.3.101.110
        0x04, 0x22, // OCTET STRING, 34 bytes
        0x04, 0x20, // OCTET STRING, 32 bytes
    ]);
    der.extend_from_slice(key_bytes);

    pem_rfc7468::encode_string("PRIVATE KEY", pem_rfc7468::LineEnding::default(), &der)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_encode_x25519_public_key() {
        // Test key from the hpke.rs tests
        let key_bytes =
            hex::decode("656c8151e6856ad5f76b7fce9ea1658e81210141e3acb803273cdce44a91fe1a")
                .unwrap();
        let pem = encode_x25519_public_key(&key_bytes).unwrap();

        // Should match OpenSSL format
        assert!(pem.contains("-----BEGIN PUBLIC KEY-----"));
        assert!(pem.contains("-----END PUBLIC KEY-----"));
        assert!(pem.contains("MCowBQYDK2VuAyEAZWyBUeaFatX3a3/OnqFljoEhAUHjrLgDJzzc5EqR/ho="));
    }

    #[test]
    fn test_encode_x25519_private_key() {
        // Test key from the hpke.rs tests
        let key_bytes =
            hex::decode("f027fda41db96155e5502519017bf6c06ecd022da855aee85749e01efbfd427b")
                .unwrap();
        let pem = encode_x25519_private_key(&key_bytes).unwrap();

        // Should match OpenSSL format
        assert!(pem.contains("-----BEGIN PRIVATE KEY-----"));
        assert!(pem.contains("-----END PRIVATE KEY-----"));
        assert!(pem.contains("MC4CAQAwBQYDK2VuBCIEIPAn/aQduWFV5VAlGQF79sBuzQItqFWu6FdJ4B77/UJ7"));
    }
}
