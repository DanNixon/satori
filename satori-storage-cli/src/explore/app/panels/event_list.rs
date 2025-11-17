use super::{
    super::{
        super::table_scroll::TableScrollState, App, KeyEventResult, SharedEvent, border_style,
        highlight_style,
    },
    PanelOperations,
};
use async_trait::async_trait;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Rect},
    style::{Modifier, Style},
    widgets::{Block, Borders, Cell, Row, Table},
};
use rayon::prelude::*;
use satori_common::EventMetadata;
use satori_storage::Provider;

pub(crate) struct EventListPanel {
    active: bool,
    storage: Provider,
    state: TableScrollState,
    event_metadata_cache: Vec<EventMetadata>,
    selected_event: SharedEvent,
}

#[async_trait]
impl PanelOperations for EventListPanel {
    fn active(&self) -> bool {
        self.active
    }

    fn set_active(&mut self, active: bool) {
        self.active = active;
    }

    fn update(&mut self) {}

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
                KeyEventResult::UpdateData
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
                KeyEventResult::UpdateData
            }

            _ => KeyEventResult::Noop,
        }
    }
}

impl EventListPanel {
    pub(crate) fn new(selected_event: SharedEvent, storage: Provider) -> Self {
        Self {
            active: true,
            storage,
            state: Default::default(),
            event_metadata_cache: Default::default(),
            selected_event,
        }
    }

    pub(crate) async fn refresh_events(&mut self) {
        self.state.clear_data();
        *self.selected_event.lock().unwrap() = None;

        if let Ok(events) = self.storage.list_events().await {
            self.event_metadata_cache = events
                .par_iter()
                .map(|p| EventMetadata::from_filename(p))
                .filter_map(|i| i.ok())
                .collect();

            // Sort by timestamp, newest first
            self.event_metadata_cache
                .sort_by(|a, b| b.timestamp.partial_cmp(&a.timestamp).unwrap());

            self.state.set_data_length(self.event_metadata_cache.len());
        }
    }

    async fn select(&mut self) {
        if let Some(i) = self.state.state().selected() {
            *self.selected_event.lock().unwrap() = Some(
                self.storage
                    .get_event(&self.event_metadata_cache[i].filename())
                    .await
                    .unwrap(),
            );
        }
    }
}

pub(crate) fn render(f: &mut Frame, app: &mut App, area: Rect) {
    let header_cells = ["Timestamp", "ID"].iter().map(|h| Cell::from(*h));

    let header = Row::new(header_cells)
        .style(Style::default().add_modifier(Modifier::UNDERLINED))
        .height(1);

    let rows = app.event_list.event_metadata_cache.iter().map(|item| {
        Row::new(vec![
            Cell::from(item.timestamp.to_string()),
            Cell::from(item.id.clone()),
        ])
        .height(1)
    });

    let active = app.event_list.active();

    let table = Table::new(
        rows,
        &[Constraint::Percentage(40), Constraint::Percentage(60)],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(border_style(active))
            .title("Events"),
    )
    .row_highlight_style(highlight_style(active));

    f.render_stateful_widget(table, area, app.event_list.state.state());
}
