use satori_testing_utils::{DummyHlsServer, DummyStreamParams, MinioDriver};
use std::{
    fs::File,
    io::{Read, Write},
    time::Duration,
};
use tempfile::NamedTempFile;

#[tokio::test]
#[ignore]
async fn two() {
    let minio = MinioDriver::default();
    minio.wait_for_ready().await;
    minio.set_credential_env_vars();
    let s3_bucket = minio.create_bucket("satori").await;

    let mut stream_1 = DummyHlsServer::new(
        "stream 1".to_string(),
        DummyStreamParams::new_ending_now(Duration::from_secs(6), 100).into(),
    )
    .await;

    let event_processor_state_store = tempfile::tempdir().unwrap();

    let archiver_config_file = {
        let contents = format!(
            indoc::indoc!(
                r#"
                kind = "s3"
                bucket = "satori"
                region = ""
                endpoint = "{}"
                "#
            ),
            minio.endpoint(),
        );

        let file = NamedTempFile::new().unwrap();
        file.as_file().write_all(contents.as_bytes()).unwrap();
        file
    };

    let satori_archiver = satori_testing_utils::CargoBinaryRunner::new(
        "satori-archiver".to_string(),
        vec![
            "--storage".to_string(),
            archiver_config_file.path().display().to_string(),
            "--api-address".to_string(),
            "127.0.0.1:8001".to_string(),
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

    let event_processor_config_file = {
        let contents = format!(
            indoc::indoc!(
                r#"
                state_store = "{}"
                event_process_interval = 10  # seconds
                archive_retry_interval = 60  # seconds
                archive_failed_task_ttl = 600  # seconds
                event_ttl = 5  # seconds
                storage_api_urls = [ "http://127.0.0.1:8001" ]

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
            event_processor_state_store.path().display(),
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
    satori_testing_utils::wait_for_url("http://localhost:8000", Duration::from_secs(600))
        .await
        .expect("event processor should be running");

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
    let mut event_processor_events_file = File::open(
        event_processor_state_store
            .path()
            .join("active_events.json"),
    )
    .unwrap();
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
    let mut events_file_contents = String::new();
    let mut event_processor_events_file = File::open(
        event_processor_state_store
            .path()
            .join("active_events.json"),
    )
    .unwrap();
    event_processor_events_file
        .read_to_string(&mut events_file_contents)
        .unwrap();
    assert!(events_file_contents == "[]");

    satori_event_processor.stop();
    satori_archiver.stop();

    stream_1.stop().await;
}
