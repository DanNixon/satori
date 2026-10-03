use tokio::time::{Duration, Instant};
use tracing::{error, info};

#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq)]
pub enum WaitForUrlError {
    #[error("timed out waiting for URL")]
    Timeout,
}

pub async fn wait_for_url(url: &str, timeout: Duration) -> Result<(), WaitForUrlError> {
    let client = reqwest::Client::new();
    let start = Instant::now();

    loop {
        let spent = Instant::now() - start;

        if spent > timeout {
            error!("Timeout waiting for URL: {}", url);
            return Err(WaitForUrlError::Timeout);
        }

        if client.get(url).send().await.is_ok() {
            info!("URL {} is available after {}s", url, spent.as_secs());
            break;
        }

        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    Ok(())
}
