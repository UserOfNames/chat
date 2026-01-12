pub mod main_panel;
pub mod popups;

use crossterm::event::KeyEvent;
use popups::Popup;

#[derive(Debug)]
pub enum Action {
    None,
    Quit,
    PushPopup(Box<dyn Popup>),
    PopPopup,
    Connect(String),
    SendMessage(String),
}

pub trait KeyHandler {
    fn handle_key(&mut self, key: KeyEvent) -> Action;
}
