use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Flex, Layout, Rect},
    style::{Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Paragraph, Widget, Wrap},
};

use super::{Action, KeyHandler, Popup};

#[derive(Debug)]
pub struct QuitPopup;

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
                Span::styled("   (y) ", Style::default().bold().blue()),
                Span::raw("Yes"),
                Span::styled("   (n) ", Style::default().bold().blue()),
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

    fn hint_size(&self) -> (u16, u16) {
        (30, 20)
    }
}
