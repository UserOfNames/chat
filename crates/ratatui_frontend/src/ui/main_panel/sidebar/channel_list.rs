use chat_backend::network_protocol::ChannelId;
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::{Style, Stylize},
    text::Line,
    widgets::{Block, Borders, List, ListItem, ListState, StatefulWidget, Widget},
};

use crate::connection_state::{ConnectionState, MessageContext};

/// Widget that displays a scrollable list of channels in the current server.
#[derive(Debug)]
pub struct ChannelList {
    list_state: ListState,
    rendered_order: Vec<ChannelId>,
    borders: Borders,
}

impl ChannelList {
    pub fn new(borders: Borders) -> Self {
        Self {
            list_state: ListState::default().with_selected(Some(0)),
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

    pub fn select(&self) -> Option<ChannelId> {
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

        let title = Line::from_iter([" [c]".bold().blue(), "hannels ".into()]);

        let block = Block::default()
            .borders(self.borders)
            .title(title)
            .title_alignment(Alignment::Center)
            .border_style(border_and_highlight_style);

        let Some(state) = state else {
            block.render(area, buf);
            return;
        };

        // TODO: Investigate ways to eliminate this requirement
        // We need our own clone of the rendering order cache for `self.select`
        self.rendered_order.clone_from(&state.channel_render_order);

        let current_channel = match &state.message_context {
            Some(MessageContext::Channel(id)) => state.channels.get(id),
            _ => None,
        }
        .map(String::as_str);

        let channels_list: Vec<ListItem> = self
            .rendered_order
            .iter()
            .map(|channel_id| {
                let channel_name = state
                    .get_channel_name(*channel_id)
                    .unwrap_or("Unknown channel");

                let line = if Some(channel_name) == current_channel {
                    Line::from(format!("◉ {channel_name}"))
                } else {
                    Line::from(channel_name)
                };

                ListItem::new(line)
            })
            .collect();

        if !channels_list.is_empty() && self.list_state.selected().is_none() {
            self.list_state.select_first();
        }

        let channels_list = List::new(channels_list)
            .block(block)
            .highlight_style(border_and_highlight_style);

        StatefulWidget::render(channels_list, area, buf, &mut self.list_state);
    }
}
