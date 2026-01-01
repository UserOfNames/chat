use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    widgets::{Block, Widget},
};
use tui_textarea::TextArea;

use super::{Action, KeyHandler, Popup, connect::ConnectPopup, quit::QuitPopup};

#[derive(Debug)]
pub struct CommandsPopup;

impl KeyHandler for CommandsPopup {
    fn handle_key(&mut self, key: KeyEvent) -> Action {
        match key.code {
            KeyCode::Esc => Action::PopPopup,
            KeyCode::Char('q') => Action::PushPopup(Box::new(QuitPopup)),

            KeyCode::Char('c') => {
                let mut ta = TextArea::default();
                ta.set_block(Block::bordered());
                Action::PushPopup(Box::new(ConnectPopup(ta)))
            }

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
