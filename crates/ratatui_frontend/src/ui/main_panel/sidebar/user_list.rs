use chat_backend::network_protocol::UserId;
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::{Style, Stylize},
    text::Line,
    widgets::{Block, Borders, List, ListItem, ListState, StatefulWidget, Widget},
};

use crate::connection_state::{MessageContext, ConnectionState};

/// Widget that displays a scrollable list of users in the current server.
#[derive(Debug)]
pub struct UserList {
    list_state: ListState,
    rendered_order: Vec<UserId>,
    borders: Borders,
}

impl UserList {
    pub fn new(borders: Borders) -> Self {
        Self {
            list_state: ListState::default(),
            rendered_order: Vec::new(),
            borders,
        }
    }

    pub fn scroll_up(&mut self) {
        self.list_state.select_previous();
    }

    pub fn scroll_down(&mut self) {
        self.list_state.select_next();
    }

    pub fn select(&self) -> Option<UserId> {
        self.list_state
            .selected()
            .and_then(|i| self.rendered_order.get(i).cloned())
    }

    pub fn render(
        &mut self,
        area: Rect,
        buf: &mut Buffer,
        state: Option<&ConnectionState>,
        focused: bool,
    ) {
        let border_and_highlight_style = if focused {
            Style::default().green()
        } else {
            Style::default()
        };

        let title = Line::from_iter([" [u]".bold().blue(), "sers ".into()]);

        let block = Block::default()
            .borders(self.borders)
            .title(title)
            .title_alignment(Alignment::Center)
            .border_style(border_and_highlight_style);

        let Some(state) = state else {
            block.render(area, buf);
            return;
        };

        // === Rebuild the rendering order cache ===
        // We want to show our user ID at the top, so we push it first and join the rest after it
        self.rendered_order.clear();
        self.rendered_order.reserve(state.users.len());

        self.rendered_order.push(state.your_id);

        for user_id in state.users.keys() {
            if user_id != &state.your_id {
                self.rendered_order.push(*user_id);
            }
        }

        let selected_user_id = match &state.message_context {
            Some(MessageContext::User(id)) => Some(id),
            _ => None,
        };

        // Needed when building our ID so the selection marker shows
        let your_id_prefix = if Some(&state.your_id) == selected_user_id {
            "◉ "
        } else {
            ""
        };

        let your_name = state.get_user_name(state.your_id).unwrap_or("YOU");

        // Special style to set our ID apart
        let your_id_line = Line::from_iter([
            your_id_prefix.into(),
            format!("{} ", your_name).blue(),
            "(you)".into(),
        ]);

        let users_list: Vec<ListItem> = self
            .rendered_order
            .iter()
            .map(|user_id| {
                let user_name = state.get_user_name(*user_id).unwrap_or("Unknown user");

                let line = if user_id == &state.your_id {
                    your_id_line.clone()
                } else if Some(user_id) == selected_user_id {
                    Line::from(format!("◉ {user_name}"))
                } else {
                    Line::from(user_name)
                };

                ListItem::new(line)
            })
            .collect();

        if !users_list.is_empty() && self.list_state.selected().is_none() {
            self.list_state.select_first();
        }

        let users_list = List::new(users_list)
            .block(block)
            .highlight_style(border_and_highlight_style);

        StatefulWidget::render(users_list, area, buf, &mut self.list_state);
    }
}
