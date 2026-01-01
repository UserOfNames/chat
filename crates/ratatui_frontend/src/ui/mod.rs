pub mod focus;
pub mod messages;
pub mod popups;
pub mod sidebar;

use crossterm::event::KeyEvent;
use focus::Focus;
use popups::Popup;

#[derive(Debug)]
pub enum Action {
    None,
    Quit,
    PushPopup(Box<dyn Popup>),
    PopPopup,
    ChangeFocus(Focus),
    ForwardToInput(KeyEvent),
}

pub trait KeyHandler {
    fn handle_key(&self, key: KeyEvent) -> Action;
}
