use satori_common::mqtt::PublishExt;
use satori_testing_utils::{DummyHlsServer, DummyStreamParams, MosquittoDriver, TestMqttClient};
use std::{io::Write, time::Duration};
use tempfile::NamedTempFile;

const MQTT_TOPIC: &str = "satori";

#[tokio::test]
#[ignore]
async fn trigger() {
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

    let mut stream_2 = DummyHlsServer::new(
        "stream 2".to_string(),
        DummyStreamParams::new("2023-01-01T00:00:01Z", Duration::from_secs(6), 100).into(),
    )
    .await;

    let mut stream_3 = DummyHlsServer::new(
        "stream 3".to_string(),
        DummyStreamParams::new("2023-01-01T00:00:02Z", Duration::from_secs(6), 100).into(),
    )
    .await;

    let event_processor_events_file = NamedTempFile::new().unwrap();

    let event_processor_config_file = {
        let contents = format!(
            indoc::indoc!(
                r#"
                event_file = "{}"
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
                cameras = ["camera1", "camera2", "camera3"]
                reason = "Unknown"
                pre = 60
                post = 60

                [[cameras]]
                name = "camera1"
                url = "{}"

                [[cameras]]
                name = "camera2"
                url = "{}"

                [[cameras]]
                name = "camera3"
                url = "{}"
                "#
            ),
            event_processor_events_file.path().display(),
            mosquitto.port(),
            stream_1.stream_address(),
            stream_2.stream_address(),
            stream_3.stream_address(),
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
        vec![],
    );

    // Wait for the event processor to start
    satori_testing_utils::wait_for_url("http://localhost:9090", Duration::from_secs(600))
        .await
        .expect("event processor should be running");

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

    // Trigger with satorictl
    satori_testing_utils::CargoBinaryRunner::new(
        "satorictl".to_string(),
        vec![
            "trigger".to_string(),
            "--mqtt".to_string(),
            ctl_mqtt_config_file.path().display().to_string(),
            "--id".to_string(),
            "test".to_string(),
            "--timestamp".to_string(),
            "2023-01-01T00:02:15+00:00".to_string(),
            "--reason".to_string(),
            "test".to_string(),
            "--camera".to_string(),
            "camera1".to_string(),
            "--camera".to_string(),
            "camera3".to_string(),
            "--pre".to_string(),
            "50".to_string(),
            "--post".to_string(),
            "30".to_string(),
        ],
        vec![],
    )
    .wait()
    .await;

    // The event trigger message should be received
    assert_eq!(
        mqtt_client
            .wait_for_message(Duration::from_secs(5))
            .await
            .unwrap()
            .try_payload_str()
            .unwrap(),
        r#"{"kind":"trigger_command","data":{"id":"test","timestamp":"2023-01-01T00:02:15Z","cameras":["camera1","camera3"],"reason":"test","pre":50,"post":30}}"#,
    );

    // Segment archive command for camera1 should be sent
    assert_eq!(
        mqtt_client
            .wait_for_message(Duration::from_secs(5))
            .await
            .unwrap()
            .try_payload_str()
            .unwrap(),
        format!(
            r#"{{"kind":"archive_command","data":{{"kind":"segments","data":{{"camera_name":"camera1","camera_url":"{}","segment_list":["2023-01-01T00_01_24+0000.ts","2023-01-01T00_01_30+0000.ts","2023-01-01T00_01_36+0000.ts","2023-01-01T00_01_42+0000.ts","2023-01-01T00_01_48+0000.ts","2023-01-01T00_01_54+0000.ts","2023-01-01T00_02_00+0000.ts","2023-01-01T00_02_06+0000.ts","2023-01-01T00_02_12+0000.ts","2023-01-01T00_02_18+0000.ts","2023-01-01T00_02_24+0000.ts","2023-01-01T00_02_30+0000.ts","2023-01-01T00_02_36+0000.ts","2023-01-01T00_02_42+0000.ts"]}}}}}}"#,
            stream_1.stream_address()
        )
    );

    // Segment archive command for camera3 should be sent
    assert_eq!(
        mqtt_client
            .wait_for_message(Duration::from_secs(5))
            .await
            .unwrap()
            .try_payload_str()
            .unwrap(),
        format!(
            r#"{{"kind":"archive_command","data":{{"kind":"segments","data":{{"camera_name":"camera3","camera_url":"{}","segment_list":["2023-01-01T00_01_20+0000.ts","2023-01-01T00_01_26+0000.ts","2023-01-01T00_01_32+0000.ts","2023-01-01T00_01_38+0000.ts","2023-01-01T00_01_44+0000.ts","2023-01-01T00_01_50+0000.ts","2023-01-01T00_01_56+0000.ts","2023-01-01T00_02_02+0000.ts","2023-01-01T00_02_08+0000.ts","2023-01-01T00_02_14+0000.ts","2023-01-01T00_02_20+0000.ts","2023-01-01T00_02_26+0000.ts","2023-01-01T00_02_32+0000.ts","2023-01-01T00_02_38+0000.ts","2023-01-01T00_02_44+0000.ts"]}}}}}}"#,
            stream_3.stream_address()
        ),
    );

    // Event metadata archive command should be sent
    assert_eq!(
        mqtt_client
            .wait_for_message(Duration::from_secs(5))
            .await
            .unwrap()
            .try_payload_str()
            .unwrap(),
        r#"{"kind":"archive_command","data":{"kind":"event_metadata","data":{"metadata":{"id":"test","timestamp":"2023-01-01T00:02:15Z"},"reasons":[{"timestamp":"2023-01-01T00:02:15Z","reason":"test"}],"start":"2023-01-01T00:01:25Z","end":"2023-01-01T00:02:45Z","cameras":[{"name":"camera1","segment_list":["2023-01-01T00_01_24+0000.ts","2023-01-01T00_01_30+0000.ts","2023-01-01T00_01_36+0000.ts","2023-01-01T00_01_42+0000.ts","2023-01-01T00_01_48+0000.ts","2023-01-01T00_01_54+0000.ts","2023-01-01T00_02_00+0000.ts","2023-01-01T00_02_06+0000.ts","2023-01-01T00_02_12+0000.ts","2023-01-01T00_02_18+0000.ts","2023-01-01T00_02_24+0000.ts","2023-01-01T00_02_30+0000.ts","2023-01-01T00_02_36+0000.ts","2023-01-01T00_02_42+0000.ts"]},{"name":"camera3","segment_list":["2023-01-01T00_01_20+0000.ts","2023-01-01T00_01_26+0000.ts","2023-01-01T00_01_32+0000.ts","2023-01-01T00_01_38+0000.ts","2023-01-01T00_01_44+0000.ts","2023-01-01T00_01_50+0000.ts","2023-01-01T00_01_56+0000.ts","2023-01-01T00_02_02+0000.ts","2023-01-01T00_02_08+0000.ts","2023-01-01T00_02_14+0000.ts","2023-01-01T00_02_20+0000.ts","2023-01-01T00_02_26+0000.ts","2023-01-01T00_02_32+0000.ts","2023-01-01T00_02_38+0000.ts","2023-01-01T00_02_44+0000.ts"]}]}}}"#
    );

    // There should be no more MQTT messages at this point
    assert!(mqtt_client
        .wait_for_message(Duration::from_secs(5))
        .await
        .is_err());

    mqtt_client.stop().await;

    satori_event_processor.stop();

    stream_1.stop().await;
    stream_2.stop().await;
    stream_3.stop().await;
}
