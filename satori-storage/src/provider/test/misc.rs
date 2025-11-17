use crate::Provider;

pub(crate) async fn test_init(provider: Provider) {
    assert!(provider.list_events().await.unwrap().is_empty());
    assert!(provider.list_cameras().await.unwrap().is_empty());
}
