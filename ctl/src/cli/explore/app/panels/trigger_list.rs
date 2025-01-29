use super::{
    super::{border_style, highlight_style, App, KeyEventResult, SharedEvent},
    PanelOperations,
};
use crate::cli::explore::table_scroll::TableScrollState;
use async_trait::async_trait;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    backend::Backend,
    layout::{Constraint, Rect},
    style::{Modifier, Style},
    widgets::{Block, Borders, Cell, Row, Table},
    Frame,
};

pub(crate) struct TriggerListPanel {
    active: bool,
    pub state: TableScrollState,
    selected_event: SharedEvent,
}

#[async_trait]
impl PanelOperations for TriggerListPanel {
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
                self.state.set_data_length(event.reasons.len());
            }
        }
    }

    async fn handle_keys(&mut self, event: KeyEvent) -> KeyEventResult {
        match event.code {
            KeyCode::Home => self.state.home(),
            KeyCode::End => self.state.end(),

            KeyCode::Char('j') => self.state.down(),
            KeyCode::Char('k') => self.state.up(),

            KeyCode::Down => self.state.down(),
            KeyCode::Up => self.state.up(),

            _ => {}
        };
        KeyEventResult::Noop
    }
}

impl TriggerListPanel {
    pub(crate) fn new(selected_event: SharedEvent) -> Self {
        Self {
            active: false,
            state: Default::default(),
            selected_event,
        }
    }
}

pub(crate) fn render<B: Backend>(f: &mut Frame<B>, app: &mut App, area: Rect) {
    let header_cells = ["Timestamp", "Reason"].iter().map(|h| Cell::from(*h));

    let header = Row::new(header_cells)
        .style(Style::default().add_modifier(Modifier::UNDERLINED))
        .height(1);

    let rows = match &*app.trigger_list.selected_event.lock().unwrap() {
        None => vec![],
        Some(event) => {
            let mut reasons = event.reasons.clone();
            // Sort by timestamp, newest first
            reasons.sort_by(|a, b| b.timestamp.partial_cmp(&a.timestamp).unwrap());

            reasons
                .iter()
                .map(|trigger| {
                    Row::new(vec![
                        Cell::from(trigger.timestamp.to_string()),
                        Cell::from(trigger.reason.clone()),
                    ])
                    .height(1)
                })
                .collect()
        }
    };

    let active = app.trigger_list.active();

    let table = Table::new(rows)
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border_style(active))
                .title("Triggers"),
        )
        .highlight_style(highlight_style(active))
        .widths(&[Constraint::Percentage(40), Constraint::Percentage(60)]);

    f.render_stateful_widget(table, area, app.trigger_list.state.state());
}
