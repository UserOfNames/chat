pub mod main_panel;
pub mod popups;

use chat_backend::SendMessage;
use crossterm::event::KeyEvent;

use popups::Popup;

#[derive(Debug)]
pub enum Action {
    None,
    Quit,
    PushPopup(Box<dyn Popup>),
    PopPopup,
    Connect(String, Option<u16>),
    SendMessage(SendMessage),
}

pub trait KeyHandler {
    fn handle_key(&mut self, key: KeyEvent) -> Action;
}
