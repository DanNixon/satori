use satori_testing_utils::{
    DummyHlsServer, DummyStreamParams, MinioDriver, MosquittoDriver, TestMqttClient,
};
use std::{
    io::{Read, Write},
    time::Duration,
};
use tempfile::NamedTempFile;

const MQTT_TOPIC: &str = "satori";

#[tokio::test]
#[ignore]
async fn two() {
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
        DummyStreamParams::new_ending_now(Duration::from_secs(6), 100).into(),
    )
    .await;

    let mut event_processor_events_file = NamedTempFile::new().unwrap();

    let event_processor_config_file = {
        let contents = format!(
            indoc::indoc!(
                r#"
                event_file = "{}"
                interval = 10
                event_ttl = 5

                [mqtt]
                broker = "{}"
                client_id = "satori-event-processor"
                username = "test"
                password = ""
                topic = "satori"

                [triggers.fallback]
                cameras = ["camera1"]
                reason = "Unknown"
                pre = 60
                post = 60

                [[cameras]]
                name = "camera1"
                url = "{}/stream.m3u8"
                "#
            ),
            event_processor_events_file.path().display(),
            mosquitto.address(),
            stream_1.address(),
        );

        let file = NamedTempFile::new().unwrap();
        file.as_file().write_all(contents.as_bytes()).unwrap();
        file
    };

    let satori_event_processor = satori_testing_utils::cargo::CargoBinaryRunner::new(
        "satori-event-processor".to_string(),
        vec![
            "--config".to_string(),
            event_processor_config_file.path().display().to_string(),
            "--observability-address".to_string(),
            "127.0.0.1:9090".to_string(),
        ],
        vec![],
    );

    // Wait for the event processor to start
    satori_testing_utils::wait_for_url("http://localhost:9090", Duration::from_secs(600))
        .await
        .expect("event processor should be running");

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
                broker = "{}"
                client_id = "satori-archiver-s3"
                username = "test"
                password = ""
                topic = "satori"

                [[cameras]]
                name = "camera1"
                url = "{}/stream.m3u8"
                "#
            ),
            archiver_queue_file.path().display(),
            minio.endpoint(),
            mosquitto.address(),
            stream_1.address(),
        );

        let file = NamedTempFile::new().unwrap();
        file.as_file().write_all(contents.as_bytes()).unwrap();
        file
    };

    let satori_archiver = satori_testing_utils::cargo::CargoBinaryRunner::new(
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

    // Trigger an event
    mqtt_client
        .client()
        .publish(
            MQTT_TOPIC,
            rumqttc::QoS::ExactlyOnce,
            false,
            r#"{"kind": "trigger_command", "data": {"id": "test", "reason": "test", "cameras": ["camera1"], "pre": 50, "post": 5 }}"#.to_string(),
        )
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_secs(2)).await;

    // Ensure event state file is not empty
    let mut events_file_contents = String::new();
    event_processor_events_file
        .read_to_string(&mut events_file_contents)
        .unwrap();
    assert!(events_file_contents != "[]");

    tokio::time::sleep(Duration::from_secs(2)).await;

    // Check event metadata is stored in S3
    let s3_events = s3_bucket
        .list("events/".to_string(), Some("/".to_string()))
        .await
        .unwrap();
    assert_eq!(s3_events[0].contents.len(), 1);

    // Check segments are stored in S3
    let s3_segments_camera1 = s3_bucket
        .list("segments/camera1/".to_string(), Some("/".to_string()))
        .await
        .unwrap();
    assert_eq!(s3_segments_camera1[0].contents.len(), 8);

    // Wait for event to expire
    // <= post + ttl + interval
    tokio::time::sleep(Duration::from_secs(20)).await;

    // Ensure event state file is empty
    assert!(events_file_contents != "[]");

    mqtt_client.stop().await;

    satori_event_processor.stop();
    satori_archiver.stop();

    minio.stop();
    mosquitto.stop();

    stream_1.stop().await;
}
