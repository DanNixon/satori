mod panels;

use self::panels::{
    PanelOperations, camera_list::CameraListPanel, event_list::EventListPanel,
    trigger_list::TriggerListPanel,
};
use crossterm::event::{self, Event, KeyCode};
use ratatui::{
    Frame, Terminal,
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};
use satori_storage::Provider;
use std::{
    io,
    sync::{Arc, Mutex},
};

fn border_style(active: bool) -> Style {
    if active {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    }
}

fn highlight_style(active: bool) -> Style {
    if active {
        Style::default()
            .add_modifier(Modifier::REVERSED)
            .fg(Color::Yellow)
    } else {
        Style::default().add_modifier(Modifier::REVERSED)
    }
}

enum KeyEventResult {
    Quit,
    Noop,
    UpdateData,
    ClearTerminal,
}

pub(super) async fn run<B: Backend>(terminal: &mut Terminal<B>, mut app: App) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui(f, &mut app))?;

        if let Event::Key(key) = event::read()? {
            let result = match key.code {
                KeyCode::Char('q') => KeyEventResult::Quit,
                KeyCode::Esc => KeyEventResult::Quit,

                KeyCode::Tab => {
                    app.tab();
                    KeyEventResult::Noop
                }

                _ => {
                    if app.event_list.active() {
                        app.event_list.handle_keys(key).await
                    } else if app.trigger_list.active() {
                        app.trigger_list.handle_keys(key).await
                    } else if app.camera_list.active() {
                        app.camera_list.handle_keys(key).await
                    } else {
                        KeyEventResult::Noop
                    }
                }
            };

            match result {
                KeyEventResult::Quit => {
                    return Ok(());
                }
                KeyEventResult::Noop => {}
                KeyEventResult::UpdateData => {
                    app.event_list.update();
                    app.trigger_list.update();
                    app.camera_list.update();
                }
                KeyEventResult::ClearTerminal => {
                    terminal.clear().unwrap();
                }
            }
        }
    }
}

type SharedEvent = Arc<Mutex<Option<satori_common::Event>>>;

pub(crate) struct App {
    event_list: EventListPanel,
    trigger_list: TriggerListPanel,
    camera_list: CameraListPanel,

    selected_event: SharedEvent,
}

impl App {
    pub(super) async fn new(storage: Provider) -> App {
        let selected_event = SharedEvent::default();

        let mut event_list = EventListPanel::new(selected_event.clone(), storage.clone());
        event_list.refresh_events().await;

        App {
            event_list,
            trigger_list: TriggerListPanel::new(selected_event.clone()),
            camera_list: CameraListPanel::new(selected_event.clone(), storage),
            selected_event,
        }
    }

    fn tab(&mut self) {
        if self.event_list.active() {
            self.event_list.set_active(false);
            self.trigger_list.set_active(true);
        } else if self.trigger_list.active() {
            self.trigger_list.set_active(false);
            self.camera_list.set_active(true);
        } else if self.camera_list.active() {
            self.camera_list.set_active(false);
            self.event_list.set_active(true);
        }
    }
}

fn ui(f: &mut Frame, app: &mut App) {
    let rects = Layout::default()
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
        .direction(Direction::Horizontal)
        .split(f.area());

    panels::event_list::render(f, app, rects[0]);
    render_right_pane(f, app, rects[1]);
}

fn render_right_pane(f: &mut Frame, app: &mut App, area: Rect) {
    let event_info_pane_height = 6;
    let app_info_pane_height = 7;

    let remaining_height =
        area.bottom() - area.top() - event_info_pane_height - app_info_pane_height;

    let rects1 = Layout::default()
        .constraints(
            [
                Constraint::Length(event_info_pane_height),
                Constraint::Length(remaining_height),
                Constraint::Length(app_info_pane_height),
            ]
            .as_ref(),
        )
        .split(area);

    let rects2 = Layout::default()
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
        .split(rects1[1]);

    render_event_info_pane(f, app, rects1[0]);
    panels::trigger_list::render(f, app, rects2[0]);
    panels::camera_list::render(f, app, rects2[1]);
    render_app_info_pane(f, app, rects1[2]);
}

fn render_event_info_pane(f: &mut Frame, app: &mut App, area: Rect) {
    let text = match &*app.selected_event.lock().unwrap() {
        None => vec![],
        Some(event) => {
            vec![
                Line::from(vec![
                    Span::raw("ID        : "),
                    Span::raw(event.metadata.id.clone()),
                ]),
                Line::from(vec![
                    Span::raw("Timestamp : "),
                    Span::raw(format!("{}", event.metadata.timestamp)),
                ]),
                Line::from(vec![
                    Span::raw("Start     : "),
                    Span::raw(format!("{}", event.start)),
                ]),
                Line::from(vec![
                    Span::raw("End       : "),
                    Span::raw(event.end.to_string()),
                ]),
            ]
        }
    };

    let info_text = Paragraph::new(text)
        .style(Style::default())
        .block(Block::default().borders(Borders::ALL).title("Event Info"));

    f.render_widget(info_text, area);
}

fn render_app_info_pane(f: &mut Frame, _: &mut App, area: Rect) {
    let title = Line::from(vec![
        Span::styled("satorictl", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(" "),
        Span::raw(satori_common::version!()),
    ]);

    let text = vec![
        Line::from(vec![Span::raw("q/Esc        : quit")]),
        Line::from(vec![Span::raw("Tab          : cycle pane")]),
        Line::from(vec![Span::raw("j/Down, k/Up : scroll list")]),
        Line::from(vec![Span::raw("Home, End    : jump to start/end of list")]),
        Line::from(vec![Span::raw("l/Enter      : select")]),
    ];

    let info_text = Paragraph::new(text)
        .style(Style::default())
        .block(Block::default().borders(Borders::ALL).title(title));

    f.render_widget(info_text, area);
}
