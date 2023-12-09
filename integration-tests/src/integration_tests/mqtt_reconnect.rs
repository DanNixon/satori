use satori_common::mqtt::PublishExt;
use satori_testing_utils::{
    DummyHlsServer, DummyStreamParams, MinioDriver, MosquittoDriver, TestMqttClient,
};
use std::{io::Write, time::Duration};
use tempfile::NamedTempFile;

const MQTT_TOPIC: &str = "satori";

#[tokio::test]
#[ignore]
async fn mqtt_reconnect() {
    let minio = MinioDriver::default();
    minio.wait_for_ready().await;
    minio.set_credential_env_vars();
    let s3_bucket = minio.create_bucket("satori").await;

    // Initially start Mosquitto
    let mosquitto = MosquittoDriver::default();

    let mut stream_1 = DummyHlsServer::new(
        "stream 1".to_string(),
        DummyStreamParams::new("2023-01-01T00:00:00Z", Duration::from_secs(6), 100).into(),
    )
    .await;

    let event_processor_events_file = NamedTempFile::new().unwrap();

    let event_processor_config_file = {
        let contents = format!(
            indoc::indoc!(
                r#"
                event_file = "{}"
                interval = 10
                event_ttl = 5

                [mqtt]
                broker = "localhost"
                port = {}
                client_id = "satori-event-processor"
                username = "test"
                password = ""
                topic = "satori"

                [triggers.fallback]
                cameras = ["camera1", "camera2", "camera3"]
                reason = "Unknown"
                pre = 60
                post = 60

                [[cameras]]
                name = "camera1"
                url = "{}/stream.m3u8"
                "#
            ),
            event_processor_events_file.path().display(),
            mosquitto.port(),
            stream_1.address(),
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
            "--observability-address".to_string(),
            "127.0.0.1:9090".to_string(),
        ],
        vec![("RUST_LOG".to_string(), "debug".to_string())],
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
                interval = 10

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
                url = "{}/stream.m3u8"
                "#
            ),
            archiver_queue_file.path().display(),
            minio.endpoint(),
            mosquitto.port(),
            stream_1.address(),
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
            ("RUST_LOG".to_string(), "debug".to_string()),
        ],
    );

    // Wait for the archiver to start
    satori_testing_utils::wait_for_url("http://localhost:9091", Duration::from_secs(600))
        .await
        .expect("archiver should be running");

    // Wait a short time and stop Mosquitto
    tokio::time::sleep(Duration::from_secs(2)).await;
    let mqtt_port = mosquitto.port();
    drop(mosquitto);

    // Wait a little bit longer and start Mosquitto again (using the same port as before)
    tokio::time::sleep(Duration::from_secs(10)).await;
    let mosquitto = MosquittoDriver::with_port(mqtt_port);

    // Wait some more time for components to reconnect to Mosquitto
    // For now this must be longer than the archiver interval (this should not be the case and
    // should be looked at when moving to rumqttc)
    tokio::time::sleep(Duration::from_secs(15)).await;

    let mut mqtt_client = TestMqttClient::new(mosquitto.port()).await;
    mqtt_client
        .client()
        .subscribe(MQTT_TOPIC, rumqttc::QoS::ExactlyOnce)
        .await
        .unwrap();

    // Trigger an event
    mqtt_client
        .client()
        .publish(
            MQTT_TOPIC,
            rumqttc::QoS::ExactlyOnce,
            false,
            r#"{"kind": "trigger_command", "data": {"id": "test", "timestamp": "2023-01-01T00:02:15Z", "reason": "test", "cameras": ["camera1"], "pre": 50, "post": 30 }}"#.to_string(),
        )
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_secs(2)).await;

    // The event trigger message should be received
    assert_eq!(
        mqtt_client
            .pop_message()
            .unwrap()
            .try_payload_str()
            .unwrap(),
        r#"{"kind": "trigger_command", "data": {"id": "test", "timestamp": "2023-01-01T00:02:15Z", "reason": "test", "cameras": ["camera1"], "pre": 50, "post": 30 }}"#,
    );

    // Segment archive command for camera1 should be sent
    assert_eq!(
        mqtt_client
            .pop_message()
            .unwrap()
            .try_payload_str()
            .unwrap(),
        r#"{"kind":"archive_command","data":{"kind":"segments","data":{"name":"camera1","segment_list":["2023-01-01T00_01_24+0000.ts","2023-01-01T00_01_30+0000.ts","2023-01-01T00_01_36+0000.ts","2023-01-01T00_01_42+0000.ts","2023-01-01T00_01_48+0000.ts","2023-01-01T00_01_54+0000.ts","2023-01-01T00_02_00+0000.ts","2023-01-01T00_02_06+0000.ts","2023-01-01T00_02_12+0000.ts","2023-01-01T00_02_18+0000.ts","2023-01-01T00_02_24+0000.ts","2023-01-01T00_02_30+0000.ts","2023-01-01T00_02_36+0000.ts","2023-01-01T00_02_42+0000.ts"]}}}"#
    );

    // Event metadata archive command should be sent
    assert_eq!(
        mqtt_client
            .pop_message()
            .unwrap()
            .try_payload_str()
            .unwrap(),
        r#"{"kind":"archive_command","data":{"kind":"event_metadata","data":{"metadata":{"id":"test","timestamp":"2023-01-01T00:02:15Z"},"reasons":[{"timestamp":"2023-01-01T00:02:15Z","reason":"test"}],"start":"2023-01-01T00:01:25Z","end":"2023-01-01T00:02:45Z","cameras":[{"name":"camera1","segment_list":["2023-01-01T00_01_24+0000.ts","2023-01-01T00_01_30+0000.ts","2023-01-01T00_01_36+0000.ts","2023-01-01T00_01_42+0000.ts","2023-01-01T00_01_48+0000.ts","2023-01-01T00_01_54+0000.ts","2023-01-01T00_02_00+0000.ts","2023-01-01T00_02_06+0000.ts","2023-01-01T00_02_12+0000.ts","2023-01-01T00_02_18+0000.ts","2023-01-01T00_02_24+0000.ts","2023-01-01T00_02_30+0000.ts","2023-01-01T00_02_36+0000.ts","2023-01-01T00_02_42+0000.ts"]}]}}}"#
    );

    tokio::time::sleep(Duration::from_secs(1)).await;

    // Check correct event metadata is stored in S3
    let s3_event = s3_bucket
        .get_object("events/2023-01-01T00:02:15+00:00_test.json")
        .await
        .unwrap();
    let s3_event = s3_event.as_str().unwrap();
    assert_eq!(
        s3_event,
        "{\n  \"metadata\": {\n    \"id\": \"test\",\n    \"timestamp\": \"2023-01-01T00:02:15Z\"\n  },\n  \"reasons\": [\n    {\n      \"timestamp\": \"2023-01-01T00:02:15Z\",\n      \"reason\": \"test\"\n    }\n  ],\n  \"start\": \"2023-01-01T00:01:25Z\",\n  \"end\": \"2023-01-01T00:02:45Z\",\n  \"cameras\": [\n    {\n      \"name\": \"camera1\",\n      \"segment_list\": [\n        \"2023-01-01T00_01_24+0000.ts\",\n        \"2023-01-01T00_01_30+0000.ts\",\n        \"2023-01-01T00_01_36+0000.ts\",\n        \"2023-01-01T00_01_42+0000.ts\",\n        \"2023-01-01T00_01_48+0000.ts\",\n        \"2023-01-01T00_01_54+0000.ts\",\n        \"2023-01-01T00_02_00+0000.ts\",\n        \"2023-01-01T00_02_06+0000.ts\",\n        \"2023-01-01T00_02_12+0000.ts\",\n        \"2023-01-01T00_02_18+0000.ts\",\n        \"2023-01-01T00_02_24+0000.ts\",\n        \"2023-01-01T00_02_30+0000.ts\",\n        \"2023-01-01T00_02_36+0000.ts\",\n        \"2023-01-01T00_02_42+0000.ts\"\n      ]\n    }\n  ]\n}"
    );

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
            "segments/camera1/2023-01-01T00_01_36+0000.ts",
            "segments/camera1/2023-01-01T00_01_42+0000.ts",
            "segments/camera1/2023-01-01T00_01_48+0000.ts",
            "segments/camera1/2023-01-01T00_01_54+0000.ts",
            "segments/camera1/2023-01-01T00_02_00+0000.ts",
            "segments/camera1/2023-01-01T00_02_06+0000.ts",
            "segments/camera1/2023-01-01T00_02_12+0000.ts",
            "segments/camera1/2023-01-01T00_02_18+0000.ts",
            "segments/camera1/2023-01-01T00_02_24+0000.ts",
            "segments/camera1/2023-01-01T00_02_30+0000.ts",
            "segments/camera1/2023-01-01T00_02_36+0000.ts",
            "segments/camera1/2023-01-01T00_02_42+0000.ts",
        ]
    );

    tokio::time::sleep(Duration::from_secs(1)).await;

    // There should be no more MQTT messages at this point
    assert!(mqtt_client.pop_message().is_none());

    mqtt_client.stop().await;

    satori_event_processor.stop();
    satori_archiver.stop();

    stream_1.stop().await;
}
