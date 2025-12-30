pub mod focus;
pub mod messages;
pub mod popup;
pub mod sidebar;

use crossterm::event::KeyEvent;
use focus::Focus;
use popup::Popup;

#[derive(Debug, Clone)]
pub enum Action {
    None,
    Quit,
    PushPopup(Popup),
    PopPopup,
    ChangeFocus(Focus),
    ForwardToInput(KeyEvent),
}

pub trait KeyHandler {
    fn handle_key(&self, key: KeyEvent) -> Action;
}
