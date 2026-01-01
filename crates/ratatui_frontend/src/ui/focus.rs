use crossterm::event::{KeyCode, KeyEvent};

use super::{Action, KeyHandler, popups::commands::CommandsPopup};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Focus {
    Normal,
    TextBox,
}

impl KeyHandler for Focus {
    fn handle_key(&self, key: KeyEvent) -> Action {
        match self {
            Self::Normal => match key.code {
                KeyCode::Char('i') => Action::ChangeFocus(Focus::TextBox),
                KeyCode::Esc => Action::PushPopup(Box::new(CommandsPopup)),
                _ => Action::None,
            },

            Self::TextBox => match key.code {
                KeyCode::Esc => Action::ChangeFocus(Focus::Normal),
                _ => Action::ForwardToInput(key),
            },
        }
    }
}
