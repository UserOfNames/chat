use chat_backend::ui_server_state::UIServerState;
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    widgets::{Block, Borders, List, ListState, StatefulWidget},
};

/// Widget that displays a scrollable list of users in the current server.
#[derive(Debug)]
pub struct UserList {
    list_state: ListState,
}

impl UserList {
    pub fn new() -> Self {
        Self {
            list_state: ListState::default(),
        }
    }

    pub fn render(&mut self, area: Rect, buf: &mut Buffer, state: Option<&UIServerState>) {
        let users_list: Vec<&str> = if let Some(state) = state {
            state.users.iter().map(|s| s.as_str()).collect()
        } else {
            Vec::new()
        };
        let users_list = List::new(users_list).block(
            Block::default()
                .borders(Borders::TOP)
                .title(" Users ")
                .title_alignment(Alignment::Center),
        );

        StatefulWidget::render(users_list, area, buf, &mut self.list_state);
    }
}
