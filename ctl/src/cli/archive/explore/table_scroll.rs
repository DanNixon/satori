use ratatui::widgets::TableState;

pub(crate) enum Step {
    Up(usize),
    Down(usize),
}

#[derive(Default)]
pub(crate) struct TableScrollState {
    data_length: Option<usize>,
    state: TableState,
}

impl TableScrollState {
    pub fn state(&mut self) -> &mut TableState {
        &mut self.state
    }

    pub fn set_data_length(&mut self, length: usize) {
        if let Some(selected) = self.state.selected() {
            if selected >= length {
                self.state.select(if length > 0 { Some(0) } else { None });
            }
        } else {
            self.state.select(if length > 0 { Some(0) } else { None });
        }
        self.data_length = Some(length);
    }

    pub fn clear_data(&mut self) {
        self.state.select(None);
        self.data_length = None;
    }

    pub fn home(&mut self) {
        if self.data_length.is_some() {
            self.state.select(Some(0));
        }
    }

    pub fn end(&mut self) {
        if let Some(data_length) = self.data_length {
            self.state.select(Some(data_length - 1));
        }
    }

    pub fn scroll(&mut self, step: Step, wrap: bool) {
        if let Some(data_length) = self.data_length {
            self.state.select(Some(match self.state.selected() {
                Some(i) => match step {
                    Step::Up(step) => {
                        if i < step {
                            if wrap { data_length - 1 } else { 0 }
                        } else {
                            i - step
                        }
                    }
                    Step::Down(step) => {
                        if i + step >= data_length {
                            if wrap { 0 } else { data_length - 1 }
                        } else {
                            i + step
                        }
                    }
                },
                None => 0,
            }));
        }
    }

    pub fn up(&mut self) {
        self.scroll(Step::Up(1), true);
    }

    pub fn down(&mut self) {
        self.scroll(Step::Down(1), true);
    }
}
