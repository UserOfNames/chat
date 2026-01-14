use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Rect},
    style::{Style, Stylize},
    text::Text,
    widgets::{Block, Cell, Row, Table, Widget},
};

use super::{
    Action, KeyHandler, Popup, SizeHint, SizeKind, connect::ConnectPopup, quit::QuitPopup,
};

const HEADER_STRS: [&str; 2] = ["Key", "Action"];

const ROW_STRS: [(&str, &str); 3] = [
    ("Esc", "Close this menu."),
    ("q", "Quit the application."),
    ("c", "Connect to a server."),
];

const COLUMN_SPACING: u16 = 5;

const LONGEST_KEY_STR: u16 = max_len(0);

const LONGEST_ACTION_STR: u16 = max_len(1);

const fn max_len(index: usize) -> u16 {
    let mut max = 0;
    let mut i = 0;
    while i < ROW_STRS.len() {
        let s = if index == 0 {
            ROW_STRS[i].0
        } else {
            ROW_STRS[i].1
        };
        if s.len() > max {
            max = s.len();
        }
        i += 1;
    }
    max as u16
}

#[derive(Debug)]
pub struct CommandsPopup;

impl CommandsPopup {
    pub fn create() -> Box<dyn Popup> {
        Box::new(Self)
    }
}

impl KeyHandler for CommandsPopup {
    fn handle_key(&mut self, key: KeyEvent) -> Action {
        match key.code {
            KeyCode::Esc => Action::PopPopup,
            KeyCode::Char('q') => Action::PushPopup(QuitPopup::create()),
            KeyCode::Char('c') => Action::PushPopup(ConnectPopup::create()),
            _ => Action::None,
        }
    }
}

impl Popup for CommandsPopup {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let block = Block::bordered()
            .title(" Commands ")
            .title_alignment(Alignment::Center);

        // Create the cells for the header.
        let header = HEADER_STRS.map(|s| Cell::new(s).style(Style::new().green()));
        // Create the actual header.
        let header = Row::new(header);

        let rows = ROW_STRS.map(|(key, action)| {
            Row::new([
                Cell::new(Text::from(key).alignment(Alignment::Right)).style(Style::new().blue()),
                Cell::new(action),
            ])
        });

        let widths = [Constraint::Length(LONGEST_KEY_STR), Constraint::Min(0)];

        Table::new(rows, widths)
            .header(header)
            .block(block)
            .column_spacing(COLUMN_SPACING)
            .render(area, buf);
    }

    fn hint_size(&self) -> SizeHint {
        // Extra 2 characters for the borders.
        let width = LONGEST_KEY_STR + LONGEST_ACTION_STR + COLUMN_SPACING + 2;
        // + 3 for borders and headers
        let height = (ROW_STRS.len() + 3) as u16;

        (SizeKind::Exact(width), SizeKind::Exact(height))
    }
}
