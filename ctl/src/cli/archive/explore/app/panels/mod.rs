pub(super) mod camera_list;
pub(super) mod event_list;
pub(super) mod trigger_list;

use super::KeyEventResult;
use async_trait::async_trait;
use crossterm::event::KeyEvent;

#[async_trait]
pub(super) trait PanelOperations {
    fn active(&self) -> bool;
    fn set_active(&mut self, active: bool);

    fn update(&mut self);

    async fn handle_keys(&mut self, event: KeyEvent) -> KeyEventResult;
}
