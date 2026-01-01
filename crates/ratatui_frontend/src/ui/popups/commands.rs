use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    widgets::{Block, Widget},
};

use super::{Action, KeyHandler, Popup, quit::QuitPopup};

#[derive(Debug)]
pub struct CommandsPopup;

impl KeyHandler for CommandsPopup {
    fn handle_key(&self, key: KeyEvent) -> Action {
        match key.code {
            KeyCode::Char('q') => Action::PushPopup(Box::new(QuitPopup)),
            KeyCode::Esc => Action::PopPopup,
            _ => Action::None,
        }
    }
}

impl Popup for CommandsPopup {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let block = Block::bordered()
            .title(" Commands ")
            .title_alignment(Alignment::Center);

        block.render(area, buf);
    }

    fn hint_size(&self) -> (u16, u16) {
        (60, 40)
    }
}
