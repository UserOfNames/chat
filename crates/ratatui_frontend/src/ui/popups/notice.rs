use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::{Style, Stylize},
    text::Text,
    widgets::{Block, Paragraph, Widget, Wrap},
};

use super::{Action, KeyHandler, Popup, SizeHint, SizeKind};

#[derive(Debug)]
pub enum NoticeLevel {
    Notification,
    Warning,
    Error,
}

#[derive(Debug)]
pub struct NoticePopup {
    message: String,
    level: NoticeLevel,
}

impl NoticePopup {
    pub fn create(message: String, level: NoticeLevel) -> Box<dyn Popup> {
        Box::new(Self { message, level })
    }
}

impl KeyHandler for NoticePopup {
    fn handle_key(&mut self, key: KeyEvent) -> Action {
        if key.code == KeyCode::Esc {
            Action::PopPopup
        } else {
            Action::None
        }
    }
}

impl Popup for NoticePopup {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let (border_title, border_style) = match self.level {
            NoticeLevel::Notification => (" Notification ", Style::default().green()),
            NoticeLevel::Warning => (" Warning ", Style::default().yellow()),
            NoticeLevel::Error => (" Error ", Style::default().red()),
        };

        let block = Block::bordered()
            .title(border_title)
            .title_alignment(Alignment::Center)
            .border_style(border_style);

        let text = Text::from(self.message.as_str());

        Paragraph::new(text)
            .block(block)
            .alignment(Alignment::Left)
            .wrap(Wrap { trim: true })
            .render(area, buf);
    }

    fn hint_size(&self) -> SizeHint {
        (SizeKind::Percentage(60), SizeKind::Percentage(40)) // TODO: Dynamic sizing
    }
}
