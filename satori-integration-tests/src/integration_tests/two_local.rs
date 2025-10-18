use satori_testing_utils::{DummyHlsServer, DummyStreamParams, RedpandaDriver, TestKafkaClient};
use std::{
    io::{Read, Write},
    time::Duration,
};
use tempfile::NamedTempFile;

const KAFKA_TOPIC: &str = "satori";

#[tokio::test]
#[ignore]
async fn two_local() {
    let storage_dir = tempfile::Builder::new()
        .prefix("satori_test_storage")
        .tempdir()
        .unwrap();

    let redpanda = RedpandaDriver::default();
    redpanda.wait_for_ready().await;

    let mut kafka_client = TestKafkaClient::new(redpanda.kafka_port(), KAFKA_TOPIC).await;

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
                interval = 10  # seconds
                event_ttl = 5

                [kafka]
                brokers = "localhost:{}"
                topic = "satori"
                group_id = "satori-archiver-local"

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
            event_processor_events_file.path().display(),
            redpanda.kafka_port(),
            stream_1.stream_address(),
        );

        let file = NamedTempFile::new().unwrap();
        file.as_file().write_all(contents.as_bytes()).unwrap();
        file
    };

    let satori_event_processor = satori_testing_utils::CargoBinaryRunner::new(
        "satori-event-processor".to_string(),
        vec![
            "--config".to_string(),
            event_processor_config_file.path().display().to_string(),
            "--http-server-address".to_string(),
            "127.0.0.1:8000".to_string(),
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
                interval = 10  # milliseconds

                [storage]
                kind = "local"
                path = "{}"

                [kafka]
                brokers = "localhost:{}"
                topic = "satori"
                group_id = "satori-archiver-local"
                "#
            ),
            archiver_queue_file.path().display(),
            storage_dir.path().display(),
            redpanda.kafka_port(),
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
        vec![],
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

    // Check event metadata is stored in local storage
    let events_dir = storage_dir.path().join("events");
    let event_files: Vec<_> = std::fs::read_dir(&events_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("json"))
        .collect();
    assert_eq!(event_files.len(), 1);

    // Check segments are stored in local storage
    let segments_camera1_dir = storage_dir.path().join("segments").join("camera1");
    let segment_files: Vec<_> = std::fs::read_dir(&segments_camera1_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("ts"))
        .collect();
    assert_eq!(segment_files.len(), 8);

    // Wait for event to expire
    // <= post + ttl + interval
    tokio::time::sleep(Duration::from_secs(20)).await;

    // Ensure event state file is empty
    assert!(events_file_contents != "[]");


    satori_event_processor.stop();
    satori_archiver.stop();

    stream_1.stop().await;
}
