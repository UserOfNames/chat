use chat_backend::ui_server_state::{MessageContext, UIServerState};
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::{Color, Style},
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

    pub fn scroll_up(&mut self) {
        self.list_state.select_previous();
    }

    pub fn scroll_down(&mut self) {
        self.list_state.select_next();
    }

    pub fn select(&self) -> Option<usize> {
        self.list_state.selected()
    }

    pub fn render(&mut self, area: Rect, buf: &mut Buffer, state: Option<&UIServerState>) {
        let users_list: Vec<String> = if let Some(state) = state {
            let selected_user = if let Some(MessageContext::User(id)) = &state.message_context {
                Some(id)
            } else {
                None
            };

            state
                .users
                .iter()
                .map(|user_id| {
                    if Some(user_id) == selected_user {
                        format!("◉ {user_id}")
                    } else {
                        user_id.clone()
                    }
                })
                .collect()
        } else {
            Vec::new()
        };

        if !users_list.is_empty() && self.list_state.selected().is_none() {
            self.list_state.select_first();
        }

        let users_list = List::new(users_list)
            .block(
                Block::default()
                    .borders(Borders::TOP)
                    .title(" Users ")
                    .title_alignment(Alignment::Center),
            )
            .highlight_style(Style::default().fg(Color::Green));

        StatefulWidget::render(users_list, area, buf, &mut self.list_state);
    }
}
