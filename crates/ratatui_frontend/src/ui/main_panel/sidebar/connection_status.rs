use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Stylize,
    widgets::{Paragraph, Widget},
};

use crate::ui_server_state::UIServerState;

/// Widget that displays the status of the current connection: whether you're connected to a server,
/// and if so, the address of that server.
#[derive(Debug)]
pub struct ConnectionStatus;

impl ConnectionStatus {
    pub fn new() -> Self {
        Self
    }

    pub fn render(&mut self, area: Rect, buf: &mut Buffer, state: Option<&UIServerState>) {
        let connection_text = if let Some(state) = state {
            Paragraph::new(state.connected_addr.to_string()).green()
        } else {
            Paragraph::new("Not connected").red()
        };

        connection_text.render(area, buf);
    }
}
