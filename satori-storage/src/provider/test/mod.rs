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
                        crate::EncryptionKey::from_bytes(&[
                            241, 82, 51, 229, 138, 74, 52, 87, 107, 37, 119, 249, 154, 52, 31, 64,
                            7, 107, 210, 255, 31, 171, 98, 129, 70, 163, 132, 168, 166, 75, 18, 88,
                        ])
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
                        crate::EncryptionKey::from_bytes(&[
                            241, 82, 51, 229, 138, 74, 52, 87, 107, 37, 119, 249, 154, 52, 31, 64,
                            7, 107, 210, 255, 31, 171, 98, 129, 70, 163, 132, 168, 166, 75, 18, 88,
                        ])
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
    use rand::RngExt;
    use satori_testing_utils::GarageDriver;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    lazy_static::lazy_static! {
        static ref GARAGE: Arc<Mutex<Option<GarageDriver>>> = Arc::new(Mutex::new(None));
    }

    #[ctor::ctor(unsafe)]
    fn init_garage() {
        let garage = GarageDriver::default();
        garage.set_credential_env_vars();
        GARAGE.try_lock().unwrap().replace(garage);
    }

    #[dtor::dtor(unsafe)]
    fn cleanup_garage() {
        let garage = GARAGE.try_lock().unwrap().take().unwrap();
        drop(garage);
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
        use super::GARAGE;

        macro_rules! test {
            ( $test:ident ) => {
                #[tokio::test]
                async fn $test() {
                    let garage = GARAGE.lock().await;
                    let garage = garage.as_ref().unwrap();

                    garage.wait_for_ready().await;

                    let bucket = super::generate_random_bucket_name();
                    garage.create_bucket(&bucket).await;

                    let storage_url = format!("s3://{}/", bucket);

                    let provider = temp_env::with_vars(
                        [
                            ("AWS_ENDPOINT", Some(garage.endpoint())),
                            ("AWS_ALLOW_HTTP", Some("true".to_string())),
                            ("AWS_REGION", Some(garage.region().to_string())),
                            ("AWS_DEFAULT_REGION", Some(garage.region().to_string())),
                        ],
                        || {
                            crate::Provider::new(
                                url::Url::parse(&storage_url).unwrap(),
                                crate::EncryptionKey::from_bytes(&[
                                    241, 82, 51, 229, 138, 74, 52, 87, 107, 37, 119, 249, 154, 52,
                                    31, 64, 7, 107, 210, 255, 31, 171, 98, 129, 70, 163, 132, 168,
                                    166, 75, 18, 88,
                                ])
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
