use std::borrow::Cow;

use chat_backend::{client_event::ReceivedMessage, network_protocol::UserId};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
    text::{Line, Text},
    widgets::{Block, List, ListItem, ListState, StatefulWidget, Widget},
};

use crate::connection_state::{ConnectionState, MessageContext};

#[derive(Debug)]
pub struct Messages {
    list_state: ListState,
}

impl Messages {
    pub fn new() -> Self {
        Self {
            list_state: ListState::default(),
        }
    }

    pub fn render(&mut self, area: Rect, buf: &mut Buffer, state: Option<&ConnectionState>) {
        let title = match state.and_then(|state| state.message_context.as_ref()) {
            Some(MessageContext::Channel(id)) => {
                let name = state
                    .expect("If this arm triggers, state is always Some")
                    .get_channel_name(*id)
                    .unwrap_or("Unknown");

                Cow::Owned(format!(" Channel: {name} "))
            }

            Some(MessageContext::User(id)) => {
                let name = state
                    .expect("If this arm triggers, state is always Some")
                    .get_user_name(*id)
                    .unwrap_or("Unknown");

                Cow::Owned(format!(" User: {name} "))
            }

            None => Cow::Borrowed(" Messages "),
        };

        let block = Block::bordered().title(title);
        let inner_area = block.inner(area);
        block.render(area, buf);

        if let Some(state) = state
            && let Some(context) = &state.message_context
            && let Some(messages) = state.messages.get(context)
        {
            let mut items: Vec<ListItem> = Vec::with_capacity(messages.len());
            let mut previous_sender: Option<UserId> = None;
            let mut is_first = true;

            for message in messages {
                let is_continuation = previous_sender == Some(message.sender_id);

                items.push(self.build_message_item(
                    message,
                    state,
                    inner_area.width,
                    is_continuation,
                    is_first,
                ));

                previous_sender = Some(message.sender_id);
                is_first = false;
            }

            let list = List::new(items).highlight_style(Style::new().reversed());
            StatefulWidget::render(list, inner_area, buf, &mut self.list_state);
        }
    }

    fn build_message_item<'a>(
        &self,
        message: &'a ReceivedMessage,
        state: &'a ConnectionState,
        max_width: u16,
        is_continuation: bool,
        is_first: bool,
    ) -> ListItem<'a> {
        let mut lines = Vec::with_capacity(8);

        // Message coalescence: skip the header if the sender didn't change
        if !is_continuation {
            // Add spacing between message clusters unless it's the first message
            if !is_first {
                lines.push(Line::raw(""));
            }

            // Specially color the user's header
            let header_style = if message.sender_id == state.your_id {
                Style::new().green()
            } else {
                Style::new().blue()
            };

            let sender_name = state
                .get_user_name(message.sender_id)
                .unwrap_or("Unknown user");

            lines.push(Line::styled(sender_name, header_style));
        }

        lines.extend(
            // TODO: Cache line wrapping, as this is an expensive operation to do every tick.
            // However, this is dependent on unresolved data modeling questions, so it must be done
            // later.
            textwrap::wrap(&message.contents, max_width as usize)
                .into_iter()
                .map(Line::from),
        );

        ListItem::new(Text::from(lines))
    }
}
