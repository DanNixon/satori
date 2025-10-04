use tokio::time::{Duration, Instant};
use tracing::warn;

struct ThrottledError<E> {
    error: E,
    first_seen: Instant,
    times_seen: usize,
}

pub struct ThrottledErrorLogger<E> {
    timeout: Duration,
    last_error: Option<ThrottledError<E>>,
}

impl<E: PartialEq + std::fmt::Display> ThrottledErrorLogger<E> {
    pub fn new(timeout: Duration) -> Self {
        Self {
            timeout,
            last_error: None,
        }
    }

    pub fn log(&mut self, error: E) -> Option<&E> {
        let now = Instant::now();
        let last_error = self.last_error.as_mut();

        match last_error {
            Some(last_error)
                if last_error.error == error && last_error.first_seen.elapsed() > self.timeout =>
            {
                warn!(
                    "Last error ({}) was (seen {} times)",
                    last_error.error, last_error.times_seen
                );
                last_error.first_seen = now;
                last_error.times_seen = 1;
                Some(&self.last_error.as_ref().unwrap().error)
            }
            Some(last_error)
                if last_error.error == error && last_error.first_seen.elapsed() <= self.timeout =>
            {
                last_error.times_seen += 1;
                None
            }
            Some(last_error) => {
                warn!(
                    "Last error ({}) was (seen {} times)",
                    last_error.error, last_error.times_seen
                );
                self.last_error = Some(ThrottledError {
                    error,
                    first_seen: now,
                    times_seen: 1,
                });
                Some(&self.last_error.as_ref().unwrap().error)
            }
            None => {
                self.last_error = Some(ThrottledError {
                    error,
                    first_seen: now,
                    times_seen: 1,
                });
                Some(&self.last_error.as_ref().unwrap().error)
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn new() {
        let te = ThrottledErrorLogger::<String>::new(Duration::from_millis(100));
        assert!(te.last_error.is_none());
    }

    #[tokio::test]
    async fn log_once() {
        let mut te = ThrottledErrorLogger::<String>::new(Duration::from_millis(100));
        assert_eq!(te.log("test".to_string()).unwrap(), "test");
    }

    #[tokio::test]
    async fn log_duplicates_fast() {
        let mut te = ThrottledErrorLogger::<String>::new(Duration::from_millis(100));
        assert_eq!(te.log("test".to_string()).unwrap(), "test");
        assert!(te.log("test".to_string()).is_none());
        assert!(te.log("test".to_string()).is_none());
        tokio::time::sleep(Duration::from_millis(95)).await;
        assert!(te.log("test".to_string()).is_none());
    }

    #[tokio::test]
    async fn log_duplicates_slow() {
        let mut te = ThrottledErrorLogger::<String>::new(Duration::from_millis(100));
        assert_eq!(te.log("test".to_string()).unwrap(), "test");
        assert!(te.log("test".to_string()).is_none());
        assert!(te.log("test".to_string()).is_none());
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(te.log("test".to_string()).unwrap(), "test");
        assert!(te.log("test".to_string()).is_none());
    }

    #[tokio::test]
    async fn log_unique_fast() {
        let mut te = ThrottledErrorLogger::<String>::new(Duration::from_millis(100));
        assert_eq!(te.log("test a".to_string()).unwrap(), "test a");
        assert_eq!(te.log("test b".to_string()).unwrap(), "test b");
        assert_eq!(te.log("test a".to_string()).unwrap(), "test a");
        tokio::time::sleep(Duration::from_millis(95)).await;
        assert_eq!(te.log("test b".to_string()).unwrap(), "test b");
    }
}
