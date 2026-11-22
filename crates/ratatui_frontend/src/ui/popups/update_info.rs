use chat_backend::network_protocol::UpdateInfo;
use crossterm::event::{KeyCode, KeyEvent};
use my_string_utils::StringExt;
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Rect},
    widgets::{Block, Widget},
};
use ratatui_textarea::TextArea;

use super::{Action, KeyHandler, Popup, SizeHint};

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
                let name = self.username_input.lines().join("").into_fast_trim();
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
        (Constraint::Percentage(30), Constraint::Length(3))
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

    fn type_string(popup: &mut dyn Popup, text: &str) {
        for c in text.chars() {
            popup.handle_key(press(KeyCode::Char(c)));
        }
    }

    #[test]
    fn test_basic_key_handling() {
        let mut popup = UpdateInfoPopup::create();

        assert!(matches!(
            popup.handle_key(press(KeyCode::Esc)),
            Action::PopPopup
        ));
    }

    #[test]
    fn test_enter_all_fields() {
        let mut popup = UpdateInfoPopup::create();

        type_string(&mut *popup, "new-username");

        let action = popup.handle_key(press(KeyCode::Enter));

        let Action::UpdateInfo(params) = action else {
            panic!("Expected Action::UpdateInfo, got {action:?}");
        };

        assert!(
            params.name.is_some(),
            "Expected params.name to be Some(_), got None"
        );
    }

    #[test]
    fn test_enter_no_fields() {
        let mut popup = UpdateInfoPopup::create();

        let action = popup.handle_key(press(KeyCode::Enter));

        let Action::UpdateInfo(params) = action else {
            panic!("Expected Action::UpdateInfo, got {action:?}");
        };

        assert!(
            params.name.is_none(),
            "Expected params.name to be None, got Some(_)"
        );
    }
}
