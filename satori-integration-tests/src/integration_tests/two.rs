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
                event_file = "/data/events.json"
                interval = 10  # seconds
                event_ttl = 5

                [mqtt]
                broker = "localhost"
                port = {}
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
                url = "{}"
                "#
            ),
            mosquitto.port(),
            stream_1.stream_address(),
        );

        let file = NamedTempFile::new().unwrap();
        file.as_file().write_all(contents.as_bytes()).unwrap();
        file
    };

    let satori_event_processor = satori_testing_utils::PodmanDriver::new(
        "localhost/satori-event-processor:latest",
        &[],
        &[],
        &[
            &format!(
                "{}:/config/config.toml:ro",
                event_processor_config_file.path().display()
            ),
            &format!(
                "{}:/data/events.json",
                event_processor_events_file.path().display()
            ),
        ],
        &[
            "--config",
            "/config/config.toml",
            "--http-server-address",
            "127.0.0.1:8000",
            "--observability-address",
            "127.0.0.1:9090",
        ],
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
                queue_file = "/data/queue.json"
                interval = 10  # milliseconds

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
                "#
            ),
            minio.endpoint(),
            mosquitto.port(),
        );

        let file = NamedTempFile::new().unwrap();
        file.as_file().write_all(contents.as_bytes()).unwrap();
        file
    };

    let satori_archiver = satori_testing_utils::PodmanDriver::new(
        "localhost/satori-archiver:latest",
        &[],
        &[
            "AWS_ACCESS_KEY_ID=minioadmin",
            "AWS_SECRET_ACCESS_KEY=minioadmin",
        ],
        &[
            &format!(
                "{}:/config/config.toml:ro",
                archiver_config_file.path().display()
            ),
            &format!("{}:/data/queue.json", archiver_queue_file.path().display()),
        ],
        &[
            "--config",
            "/config/config.toml",
            "--observability-address",
            "127.0.0.1:9091",
        ],
    );

    // Wait for the archiver to start
    satori_testing_utils::wait_for_url("http://localhost:9091", Duration::from_secs(600))
        .await
        .expect("archiver should be running");

    // Trigger an event via HTTP
    let http_client = reqwest::Client::new();
    http_client
        .post("http://localhost:8000/trigger")
        .header("Content-Type", "application/json")
        .body(r#"{"id": "test", "reason": "test", "cameras": ["camera1"], "pre": 50, "post": 5 }"#)
        .send()
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

    drop(satori_event_processor);
    drop(satori_archiver);

    stream_1.stop().await;
}
