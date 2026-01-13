use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    widgets::{Block, Widget},
};
use tui_textarea::TextArea;

use super::{Action, KeyHandler, Popup, SizeHint};

#[derive(Debug)]
pub struct ConnectPopup(TextArea<'static>);

impl ConnectPopup {
    pub fn create() -> Box<dyn Popup> {
        let mut ta = TextArea::default();
        ta.set_block(Block::bordered());
        Box::new(Self(ta))
    }
}

impl KeyHandler for ConnectPopup {
    fn handle_key(&mut self, key: KeyEvent) -> Action {
        match key.code {
            KeyCode::Esc => Action::PopPopup,

            KeyCode::Enter => {
                let addr = self.0.lines().join("");
                Action::Connect(addr)
            }

            _ => {
                self.0.input(key);
                Action::None
            }
        }
    }
}

impl Popup for ConnectPopup {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        self.0.render(area, buf);
    }

    fn hint_size(&self) -> SizeHint {
        SizeHint::Percentage(70, 10)
    }
}
