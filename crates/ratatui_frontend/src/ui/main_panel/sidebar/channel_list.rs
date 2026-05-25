use chat_backend::ui_server_state::{MessageContext, UIServerState};
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::{Color, Style},
    widgets::{Block, Borders, List, ListState, StatefulWidget},
};

/// Widget that displays a scrollable list of channels in the current server.
#[derive(Debug)]
pub struct ChannelList {
    list_state: ListState,
}

impl ChannelList {
    pub fn new() -> Self {
        Self {
            list_state: ListState::default().with_selected(Some(0)),
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
        let channels_list: Vec<String> = if let Some(state) = state {
            let current_channel = if let Some(MessageContext::Channel(id)) = &state.message_context
            {
                Some(id)
            } else {
                None
            };

            state
                .channels
                .iter()
                .map(|channel_id| {
                    if Some(channel_id) == current_channel {
                        format!("◉ {channel_id}")
                    } else {
                        channel_id.clone()
                    }
                })
                .collect()
        } else {
            Vec::new()
        };

        if !channels_list.is_empty() && self.list_state.selected().is_none() {
            self.list_state.select_first();
        }

        let channels_list = List::new(channels_list)
            .block(
                Block::default()
                    .borders(Borders::TOP)
                    .title(" Channels ")
                    .title_alignment(Alignment::Center),
            )
            .highlight_style(Style::default().fg(Color::Green));

        StatefulWidget::render(channels_list, area, buf, &mut self.list_state);
    }
}
