use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Flex, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Paragraph, Widget, Wrap},
};

use super::{Action, KeyHandler, Popup, SizeHint};

#[derive(Debug)]
pub struct QuitPopup;

impl QuitPopup {
    pub fn create() -> Box<dyn Popup> {
        Box::new(Self)
    }
}

impl KeyHandler for QuitPopup {
    fn handle_key(&mut self, key: KeyEvent) -> Action {
        match key.code {
            KeyCode::Char('y') => Action::Quit,
            KeyCode::Char('n') | KeyCode::Esc => Action::PopPopup,
            _ => Action::None,
        }
    }
}

impl Popup for QuitPopup {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        Block::bordered()
            .title(" Confirm ")
            .title_alignment(Alignment::Center)
            .render(area, buf);

        let text = vec![
            Line::from("Are you sure you want to quit?").centered(),
            Line::from(""),
            Line::from(vec![
                Span::styled("   (y) ", Style::default().blue()),
                Span::raw("Yes"),
                Span::styled("   (n) ", Style::default().blue()),
                Span::raw("No"),
            ])
            .centered(),
        ];

        let [area] = Layout::vertical([Constraint::Length(text.len() as u16)])
            .flex(Flex::Center)
            .areas(area);

        Paragraph::new(text)
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true })
            .render(area, buf);
    }

    fn hint_size(&self) -> SizeHint {
        (Constraint::Percentage(30), Constraint::Percentage(20))
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
    fn test_quit_popup_key_handling() {
        let mut popup = QuitPopup::create();

        // Yes triggers Quit
        assert!(matches!(popup.handle_key(press(KeyCode::Char('y'))), Action::Quit));

        // No or Esc triggers PopPopup
        assert!(matches!(popup.handle_key(press(KeyCode::Char('n'))), Action::PopPopup));
        assert!(matches!(popup.handle_key(press(KeyCode::Esc)), Action::PopPopup));

        // Anything else is ignored
        assert!(matches!(popup.handle_key(press(KeyCode::Enter)), Action::None));
        assert!(matches!(popup.handle_key(press(KeyCode::Char('q'))), Action::None));
    }
}
