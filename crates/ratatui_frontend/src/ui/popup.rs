use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Flex, Layout, Rect},
    style::{Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Clear, Paragraph, Widget, Wrap},
};

use super::{Action, KeyHandler};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Popup {
    Commands,
    Quit,
}

impl Widget for &Popup {
    fn render(self, area: Rect, buf: &mut Buffer) {
        Clear.render(area, buf);

        match self {
            Popup::Commands => render_command(area, buf),
            Popup::Quit => render_quit(area, buf),
        }
    }
}

impl KeyHandler for Popup {
    fn handle_key(&self, key: KeyEvent) -> Action {
        match self {
            Self::Quit => match key.code {
                KeyCode::Char('y') => Action::Quit,
                KeyCode::Char('n') | KeyCode::Esc => Action::PopPopup,
                _ => Action::None,
            },

            Self::Commands => match key.code {
                KeyCode::Char('q') => Action::PushPopup(Popup::Quit),
                KeyCode::Esc => Action::PopPopup,
                _ => Action::None,
            },
        }
    }
}

// https://ratatui.rs/examples/apps/popup/
pub fn popup_area(area: Rect, percent_x: u16, percent_y: u16) -> Rect {
    let vertical = Layout::vertical([Constraint::Percentage(percent_y)]).flex(Flex::Center);
    let horizontal = Layout::horizontal([Constraint::Percentage(percent_x)]).flex(Flex::Center);
    let [area] = vertical.areas(area);
    let [area] = horizontal.areas(area);
    area
}

fn render_command(area: Rect, buf: &mut Buffer) {
    let block = Block::bordered()
        .title(" Commands ")
        .title_alignment(Alignment::Center);

    block.render(area, buf);
}

fn render_quit(area: Rect, buf: &mut Buffer) {
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
