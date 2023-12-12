#[ctor::ctor]
fn init() {
    tracing_subscriber::fmt()
        .with_test_writer()
        .with_max_level(tracing::Level::DEBUG)
        .init();
}

mod ctl;
mod mqtt_reconnect;
mod one;
mod two;
