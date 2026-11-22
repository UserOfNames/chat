use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Rect},
    style::Style,
    text::Text,
    widgets::{Block, Cell, Row, Table, Widget},
};

use super::{
    Action, KeyHandler, Popup, SizeHint, connect::ConnectPopup, quit::QuitPopup,
    update_info::UpdateInfoPopup,
};

const HEADER_STRS: [&str; 2] = ["Key", "Action"];

const ROW_STRS: [(&str, &str); 4] = [
    ("Esc", "Close this menu."),
    ("q", "Quit the application."),
    ("c", "Connect to a server."),
    ("u", "Update your information."),
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

    usize_to_u16(max)
}

const fn usize_to_u16(x: usize) -> u16 {
    // HACK: TryFrom is unusable because const traits are still unstable
    #[expect(clippy::cast_possible_truncation)]
    {
        assert!(x <= u16::MAX as usize);
        x as u16
    }
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
            KeyCode::Char('u') => Action::PushPopup(UpdateInfoPopup::create()),
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
        let width = const { LONGEST_KEY_STR + LONGEST_ACTION_STR + COLUMN_SPACING + 2 };

        // + 3 for borders and headers
        let height = const { usize_to_u16(ROW_STRS.len() + 3) };

        (Constraint::Length(width), Constraint::Length(height))
    }
}

#[cfg(test)]
mod test {
    use crossterm::event::{KeyEventKind, KeyModifiers};

    use super::*;

    fn press(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::empty(),
            kind: KeyEventKind::Press,
            state: crossterm::event::KeyEventState::empty(),
        }
    }

    #[test]
    fn test_key_handling() {
        let mut popup = CommandsPopup::create();

        // Esc returns PopPopup
        assert!(matches!(popup.handle_key(press(KeyCode::Esc)), Action::PopPopup));

        // 'q' spawns a QuitPopup
        assert!(matches!(popup.handle_key(press(KeyCode::Char('q'))), Action::PushPopup(_)));

        // 'c' spawns a ConnectPopup
        assert!(matches!(popup.handle_key(press(KeyCode::Char('c'))), Action::PushPopup(_)));

        // 'u' spawns a UpdateInfoPopup
        assert!(matches!(popup.handle_key(press(KeyCode::Char('u'))), Action::PushPopup(_)));

        // All else is ignored
        assert!(matches!(popup.handle_key(press(KeyCode::Char('a'))), Action::None));
        assert!(matches!(popup.handle_key(press(KeyCode::Char('3'))), Action::None));
    }
}
