mod app;
mod table_scroll;

use clap::Parser;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use miette::IntoDiagnostic;
use ratatui::{Terminal, backend::CrosstermBackend};
use satori_storage::Provider;
use std::io;

/// Interactively explore contents of an archive
#[derive(Debug, Clone, Parser)]
pub(crate) struct ExploreCommand {}

impl ExploreCommand {
    pub(super) async fn execute(&self, storage: Provider) -> miette::Result<()> {
        let app = self::app::App::new(storage).await;

        setup_terminal();
        let backend = CrosstermBackend::new(io::stdout());
        let mut terminal = Terminal::new(backend).unwrap();

        let result = self::app::run(&mut terminal, app).await;

        reset_terminal();
        terminal.show_cursor().unwrap();

        result.into_diagnostic()
    }
}

fn setup_terminal() {
    enable_raw_mode().unwrap();
    execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture).unwrap();
}

fn reset_terminal() {
    disable_raw_mode().unwrap();
    execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture).unwrap();
}
