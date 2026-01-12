use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    widgets::{Block, Widget},
};

use super::{Action, KeyHandler, Popup, connect::ConnectPopup, quit::QuitPopup};

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

        block.render(area, buf);
    }

    fn hint_size(&self) -> (u16, u16) {
        (60, 40)
    }
}
