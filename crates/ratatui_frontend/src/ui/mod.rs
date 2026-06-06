pub mod main_panel;
pub mod popups;

use chat_backend::{
    client_command::ConnectParams,
    network_protocol::{ChannelId, UpdateInfo, UserId},
};
use crossterm::event::KeyEvent;

use popups::Popup;

#[derive(Debug)]
pub enum Action {
    None,
    Quit,
    PushPopup(Box<dyn Popup>),
    PopPopup,
    Connect(ConnectParams),
    SendMessage(String),
    UpdateInfo(UpdateInfo),

    YieldFocus,
    SelectChannel(ChannelId),
    SelectUser(UserId),
}

pub trait KeyHandler {
    fn handle_key(&mut self, key: KeyEvent) -> Action;
}
