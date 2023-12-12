use satori_common::mqtt::PublishExt;
use satori_testing_utils::{
    DummyHlsServer, DummyStreamParams, MinioDriver, MosquittoDriver, TestMqttClient,
};
use std::{io::Write, time::Duration};
use tempfile::NamedTempFile;

const MQTT_TOPIC: &str = "satori";

#[tokio::test]
#[ignore]
async fn debug_archive_segments() {
    let minio = MinioDriver::default();
    minio.wait_for_ready().await;
    minio.set_credential_env_vars();
    let s3_bucket = minio.create_bucket("satori").await;

    let mosquitto = MosquittoDriver::default();

    let mut mqtt_client = TestMqttClient::new(mosquitto.port()).await;
    mqtt_client
        .client()
        .subscribe(MQTT_TOPIC, rumqttc::QoS::ExactlyOnce)
        .await
        .unwrap();

    let mut stream_1 = DummyHlsServer::new(
        "stream 1".to_string(),
        DummyStreamParams::new("2023-01-01T00:00:00Z", Duration::from_secs(6), 100).into(),
    )
    .await;

    let archiver_queue_file = NamedTempFile::new().unwrap();

    let archiver_config_file = {
        let contents = format!(
            indoc::indoc!(
                r#"
                queue_file = "{}"
                interval = 30

                [storage]
                kind = "s3"
                bucket = "satori"
                region = ""
                endpoint = "{}"

                [mqtt]
                broker = "localhost"
                port = {}
                client_id = "satori-archiver-s3"
                username = "test"
                password = ""
                topic = "satori"

                [[cameras]]
                name = "camera1"
                url = "{}/"
                "#
            ),
            archiver_queue_file.path().display(),
            minio.endpoint(),
            mosquitto.port(),
            stream_1.stream_address(),
        );

        let file = NamedTempFile::new().unwrap();
        file.as_file().write_all(contents.as_bytes()).unwrap();
        file
    };

    let satori_archiver = satori_testing_utils::CargoBinaryRunner::new(
        "satori-archiver".to_string(),
        vec![
            "--config".to_string(),
            archiver_config_file.path().display().to_string(),
            "--observability-address".to_string(),
            "127.0.0.1:9091".to_string(),
        ],
        vec![
            ("AWS_ACCESS_KEY_ID".to_string(), "minioadmin".to_string()),
            (
                "AWS_SECRET_ACCESS_KEY".to_string(),
                "minioadmin".to_string(),
            ),
        ],
    );

    // Wait for the archiver to start
    satori_testing_utils::wait_for_url("http://localhost:9091", Duration::from_secs(600))
        .await
        .expect("archiver should be running");

    let ctl_mqtt_config_file = {
        let contents = format!(
            indoc::indoc!(
                r#"
                broker = "localhost"
                port = {}
                client_id = "satorictl"
                username = "test"
                password = ""
                topic = "satori"
                "#
            ),
            mosquitto.port(),
        );

        let file = NamedTempFile::new().unwrap();
        file.as_file().write_all(contents.as_bytes()).unwrap();
        file
    };

    // Trigger segment archive with satorictl
    satori_testing_utils::CargoBinaryRunner::new(
        "satorictl".to_string(),
        vec![
            "debug".to_string(),
            "--mqtt".to_string(),
            ctl_mqtt_config_file.path().display().to_string(),
            "archive-segments".to_string(),
            "--camera".to_string(),
            "camera1".to_string(),
            "--url".to_string(),
            stream_1.stream_address(),
            "2023-01-01T00_01_24+0000.ts".to_string(),
            "2023-01-01T00_01_30+0000.ts".to_string(),
        ],
        vec![],
    )
    .wait()
    .await;

    // Segment archive command for camera1 should be sent
    assert_eq!(
        mqtt_client
            .wait_for_message(Duration::from_secs(5))
            .await
            .unwrap()
            .try_payload_str()
            .unwrap(),
        format!(
            r#"{{"kind":"archive_command","data":{{"kind":"segments","data":{{"camera_name":"camera1","camera_url":"{}","segment_list":["2023-01-01T00_01_24+0000.ts","2023-01-01T00_01_30+0000.ts"]}}}}}}"#,
            stream_1.stream_address()
        )
    );

    tokio::time::sleep(Duration::from_secs(1)).await;

    // Check correct segments are stored in S3
    let s3_segments_camera1 = s3_bucket
        .list("segments/camera1/".to_string(), Some("/".to_string()))
        .await
        .unwrap();
    let s3_segments_camera1 = s3_segments_camera1[0]
        .contents
        .iter()
        .map(|s| s.key.clone())
        .collect::<Vec<_>>();
    assert_eq!(
        s3_segments_camera1,
        vec![
            "segments/camera1/2023-01-01T00_01_24+0000.ts",
            "segments/camera1/2023-01-01T00_01_30+0000.ts",
        ]
    );

    mqtt_client.stop().await;

    satori_archiver.stop();

    stream_1.stop().await;
}
