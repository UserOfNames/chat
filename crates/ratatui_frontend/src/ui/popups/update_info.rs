use chat_backend::network_protocol::UpdateInfo;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    widgets::{Block, Widget},
};
use ratatui_textarea::TextArea;

use super::{Action, KeyHandler, Popup, SizeHint, SizeKind};

#[derive(Debug)]
pub struct UpdateInfoPopup {
    username_input: TextArea<'static>,
}

impl UpdateInfoPopup {
    pub fn create() -> Box<dyn Popup> {
        let mut username_input = TextArea::default();
        let block = Block::bordered().title("Enter new username");
        username_input.set_block(block);

        Box::new(Self { username_input })
    }
}

impl KeyHandler for UpdateInfoPopup {
    fn handle_key(&mut self, key: KeyEvent) -> Action {
        match key.code {
            KeyCode::Esc => Action::PopPopup,

            KeyCode::Enter => {
                let name = self.username_input.lines().join("").trim().to_owned();
                let name = if name.is_empty() { None } else { Some(name) };

                let update_info = UpdateInfo { name };

                Action::UpdateInfo(update_info)
            }

            _ => {
                self.username_input.input(key);
                Action::None
            }
        }
    }
}

impl Popup for UpdateInfoPopup {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        self.username_input.render(area, buf);
    }

    fn hint_size(&self) -> SizeHint {
        (SizeKind::Percentage(30), SizeKind::Exact(3))
    }
}
