use tokio::time::{Duration, Instant};
use tracing::{error, info};

pub async fn wait_for_url(url: &str, timeout: Duration) -> Result<(), ()> {
    let client = reqwest::Client::new();
    let start = Instant::now();

    loop {
        let spent = Instant::now() - start;

        if spent > timeout {
            error!("Timeout waiting for URL: {}", url);
            return Err(());
        }

        if client.get(url).send().await.is_ok() {
            info!("URL {} is available after {}s", url, spent.as_secs());
            break;
        }

        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    Ok(())
}
