use super::{
    super::{
        super::{reset_terminal, setup_terminal, table_scroll::TableScrollState},
        App, KeyEventResult, SharedEvent, border_style, highlight_style,
    },
    PanelOperations,
};
use async_trait::async_trait;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Rect},
    widgets::{Block, Borders, Cell, Row, Table},
};
use satori_storage::{Provider, workflows};
use std::{fs::File, io::Write};
use tracing::info;

pub(crate) struct CameraListPanel {
    active: bool,
    storage: Provider,
    pub state: TableScrollState,
    selected_event: SharedEvent,
}

#[async_trait]
impl PanelOperations for CameraListPanel {
    fn active(&self) -> bool {
        self.active
    }

    fn set_active(&mut self, active: bool) {
        self.active = active;
    }

    fn update(&mut self) {
        match &*self.selected_event.lock().unwrap() {
            None => {
                self.state.clear_data();
            }
            Some(event) => {
                self.state.set_data_length(event.cameras.len());
            }
        }
    }

    async fn handle_keys(&mut self, event: KeyEvent) -> KeyEventResult {
        match event.code {
            KeyCode::Home => {
                self.state.home();
                KeyEventResult::Noop
            }
            KeyCode::End => {
                self.state.end();
                KeyEventResult::Noop
            }

            KeyCode::Char('j') => {
                self.state.down();
                KeyEventResult::Noop
            }
            KeyCode::Char('k') => {
                self.state.up();
                KeyEventResult::Noop
            }
            KeyCode::Char('l') => {
                self.select().await;
                KeyEventResult::ClearTerminal
            }

            KeyCode::Down => {
                self.state.down();
                KeyEventResult::Noop
            }
            KeyCode::Up => {
                self.state.up();
                KeyEventResult::Noop
            }
            KeyCode::Enter => {
                self.select().await;
                KeyEventResult::ClearTerminal
            }

            _ => KeyEventResult::Noop,
        }
    }
}

impl CameraListPanel {
    pub(crate) fn new(selected_event: SharedEvent, storage: Provider) -> Self {
        Self {
            active: false,
            storage,
            state: Default::default(),
            selected_event,
        }
    }

    async fn select(&mut self) {
        let doot = if let Some(selection) = self.state.state().selected() {
            if let Some(event) = &*self.selected_event.lock().unwrap() {
                let camera_name = Some(event.cameras[selection].name.clone());
                Some((event.clone(), camera_name))
            } else {
                None
            }
        } else {
            None
        };

        reset_terminal();

        if let Some((event, camera_name)) = doot {
            let output_filename =
                workflows::generate_video_filename(&event, camera_name.clone()).unwrap();
            info!("Saving to {}", output_filename.display());
            let mut file = File::create(&output_filename).unwrap();

            let (_, file_content) = workflows::export_event_video(
                self.storage.clone(),
                &event.metadata.get_filename(),
                camera_name,
            )
            .await
            .unwrap();
            file.write_all(&file_content).unwrap();
        }

        setup_terminal();
    }
}

pub(crate) fn render(f: &mut Frame, app: &mut App, area: Rect) {
    let rows = match &*app.camera_list.selected_event.lock().unwrap() {
        None => vec![],
        Some(event) => event
            .cameras
            .iter()
            .map(|camera| Row::new(vec![Cell::from(camera.name.to_string())]).height(1))
            .collect(),
    };

    let active = app.camera_list.active();

    let table = Table::new(rows, &[Constraint::Percentage(100)])
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border_style(active))
                .title("Cameras"),
        )
        .row_highlight_style(highlight_style(active));

    f.render_stateful_widget(table, area, app.camera_list.state.state());
}
