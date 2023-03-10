use super::KeyOperations;
use bytes::Bytes;

macro_rules! encryption_test {
    ( $test:ident, $keypair:ident ) => {
        #[test]
        fn $test() {
            let (pk, sk) = $keypair();
            crate::encryption::test::$test(pk, sk);
        }
    };
}

pub(crate) use encryption_test;

pub(crate) fn basic_round_trip(pk: impl KeyOperations, sk: impl KeyOperations) {
    let id = Bytes::from("test");
    let plaintext = Bytes::from("hello world");

    let ciphertext = pk.encrypt(id.clone(), plaintext.clone()).unwrap();

    let recovered_plaintext = sk.decrypt(id, ciphertext).unwrap();

    assert_eq!(plaintext, recovered_plaintext);
}

pub(crate) fn cannot_decrypt_without_sk(pk: impl KeyOperations, _sk: impl KeyOperations) {
    let id = Bytes::from("test");
    let plaintext = Bytes::from("hello world");

    let ciphertext = pk.encrypt(id.clone(), plaintext).unwrap();

    let result = pk.decrypt(id, ciphertext);

    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().to_string(),
        "A key that is required to perform an en/decrption operation is not provided"
    );
}

pub(crate) fn key_mismatch(pk: impl KeyOperations, sk: impl KeyOperations) {
    let id = Bytes::from("test");
    let plaintext = Bytes::from("hello world");

    let ciphertext = pk.encrypt(id.clone(), plaintext).unwrap();

    let result = sk.decrypt(id, ciphertext);

    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().to_string(),
        "HPKE error: Failed to open ciphertext"
    );
}
