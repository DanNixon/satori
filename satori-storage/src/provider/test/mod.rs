mod creation;
pub(super) use creation::*;

mod deletion;
pub(super) use deletion::*;

mod misc;
pub(super) use misc::*;

mod retrieval;
pub(super) use retrieval::*;

macro_rules! all_storage_tests {
    ( $test_macro:ident ) => {
        $test_macro!(test_add_first_event);
        $test_macro!(test_add_event);
        $test_macro!(test_add_segment_new_camera);
        $test_macro!(test_add_segment_existing_camera);

        $test_macro!(test_delete_event);
        $test_macro!(test_delete_event_filename);
        $test_macro!(test_delete_segment);
        $test_macro!(test_delete_last_segment_deletes_camera);

        $test_macro!(test_init);

        $test_macro!(test_event_getters);
        $test_macro!(test_segment_getters);
    };
}

mod inmemory {
    mod encryption_hpke {
        macro_rules! test {
            ( $test:ident ) => {
                #[tokio::test]
                async fn $test() {
                    let provider = crate::Provider::new(
                        url::Url::parse("memory:///").unwrap(),
                        toml::from_str(
                            "
kind = \"hpke\"
public_key = [227, 73, 200, 96, 32, 198, 168, 234, 95, 35, 250, 8, 195, 25, 114, 67, 2, 206, 247, 21, 255, 175, 211, 33, 232, 187, 73, 197, 167, 157, 7, 121]
private_key = [94, 205, 32, 31, 23, 53, 162, 104, 83, 164, 87, 216, 55, 121, 41, 107, 8, 236, 255, 233, 48, 52, 79, 109, 58, 254, 138, 158, 131, 204, 1, 118]
",
                        )
                        .unwrap(),
                    )
                    .unwrap();

                    crate::provider::test::$test(provider).await;
                }
            };
        }

        all_storage_tests!(test);
    }
}

mod local {
    mod encryption_hpke {
        macro_rules! test {
            ( $test:ident ) => {
                #[tokio::test]
                async fn $test() {
                    let temp_dir = tempfile::Builder::new()
                        .prefix("satori_local_storage_test")
                        .tempdir()
                        .unwrap();

                    let storage_url = format!("file://{}", temp_dir.path().display());

                    let provider = crate::Provider::new(
                        url::Url::parse(&storage_url).unwrap(),
                        toml::from_str(
                            "
kind = \"hpke\"
public_key = [227, 73, 200, 96, 32, 198, 168, 234, 95, 35, 250, 8, 195, 25, 114, 67, 2, 206, 247, 21, 255, 175, 211, 33, 232, 187, 73, 197, 167, 157, 7, 121]
private_key = [94, 205, 32, 31, 23, 53, 162, 104, 83, 164, 87, 216, 55, 121, 41, 107, 8, 236, 255, 233, 48, 52, 79, 109, 58, 254, 138, 158, 131, 204, 1, 118]
",
                        )
                        .unwrap(),
                    )
                    .unwrap();

                    crate::provider::test::$test(provider).await;
                }
            };
        }

        all_storage_tests!(test);
    }
}

mod s3 {
    use rand::Rng;
    use satori_testing_utils::MinioDriver;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    lazy_static::lazy_static! {
        static ref MINIO: Arc<Mutex<Option<MinioDriver>>> = Arc::new(Mutex::new(None));
    }

    #[ctor::ctor]
    fn init_minio() {
        let minio = MinioDriver::default();
        minio.set_credential_env_vars();
        MINIO.try_lock().unwrap().replace(minio);
    }

    #[ctor::dtor]
    fn cleanup_minio() {
        let minio = MINIO.try_lock().unwrap().take().unwrap();
        drop(minio);
    }

    fn generate_random_bucket_name() -> String {
        let id = rand::rng()
            .sample_iter(&rand::distr::Alphanumeric)
            .take(8)
            .map(char::from)
            .collect::<String>()
            .to_lowercase();

        format!("satori-storage-test-{id}")
    }

    mod encryption_hpke {
        use super::MINIO;

        macro_rules! test {
            ( $test:ident ) => {
                #[tokio::test]
                async fn $test() {
                    let minio = MINIO.lock().await;
                    let minio = minio.as_ref().unwrap();

                    minio.wait_for_ready().await;

                    let bucket = super::generate_random_bucket_name();
                    minio.create_bucket(&bucket).await;

                    let storage_url = format!("s3://{}/", bucket);

                    let provider = temp_env::with_vars(
                        [
                            ("AWS_ENDPOINT", Some(minio.endpoint())),
                            ("AWS_ALLOW_HTTP", Some("true".to_string())),
                        ],
                        || {
                            crate::Provider::new(
                                url::Url::parse(&storage_url).unwrap(),
                                toml::from_str(
                                    "
kind = \"hpke\"
public_key = [227, 73, 200, 96, 32, 198, 168, 234, 95, 35, 250, 8, 195, 25, 114, 67, 2, 206, 247, 21, 255, 175, 211, 33, 232, 187, 73, 197, 167, 157, 7, 121]
private_key = [94, 205, 32, 31, 23, 53, 162, 104, 83, 164, 87, 216, 55, 121, 41, 107, 8, 236, 255, 233, 48, 52, 79, 109, 58, 254, 138, 158, 131, 204, 1, 118]
",
                                )
                                .unwrap(),
                            )
                            .unwrap()
                        },
                    );

                    crate::provider::test::$test(provider).await;
                }
            };
        }

        all_storage_tests!(test);
    }
}
