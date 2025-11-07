#[ctor::ctor]
fn init() {
    tracing_subscriber::fmt()
        .with_test_writer()
        .with_max_level(tracing::Level::DEBUG)
        .init();
}

// MQTT reconnect test is no longer relevant after removing MQTT
// mod mqtt_reconnect;
mod one;
mod two;
mod two_local;
