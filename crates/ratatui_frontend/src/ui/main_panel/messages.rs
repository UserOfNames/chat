use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
    text::{Line, Text},
    widgets::{Block, List, ListItem, ListState, StatefulWidget, Widget},
};

use chat_backend::{
    client_event::ReceivedMessage,
    ui_server_state::{MessageContext, UIServerState},
};

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

    pub fn render(&mut self, area: Rect, buf: &mut Buffer, state: Option<&UIServerState>) {
        let title = match state.and_then(|state| state.message_context.as_ref()) {
            Some(MessageContext::Channel(id)) => {
                let name = state
                    .expect("If this arm triggers, state is always Some")
                    .get_channel_name(*id)
                    .unwrap_or("Unknown");

                format!(" Channel: {name} ")
            }

            Some(MessageContext::User(id)) => {
                let name = state
                    .expect("If this arm triggers, state is always Some")
                    .get_user_name(*id)
                    .unwrap_or("Unknown");

                format!(" User: {name} ")
            }

            None => " Messages ".to_owned(),
        };

        let block = Block::bordered().title(title);
        let inner_area = block.inner(area);
        block.render(area, buf);

        if let Some(state) = state
            && let Some(context) = &state.message_context
            && let Some(messages) = state.messages.get(context)
        {
            let items: Vec<ListItem> = messages
                .iter()
                .map(|msg| self.build_message_item(msg, state, inner_area.width))
                .collect();

            let list = List::new(items).highlight_style(Style::new().reversed());
            StatefulWidget::render(list, inner_area, buf, &mut self.list_state);
        }
    }

    fn build_message_item<'a>(
        &self,
        message: &'a ReceivedMessage,
        state: &'a UIServerState,
        max_width: u16,
    ) -> ListItem<'a> {
        let header_style = if message.sender_id == state.your_id {
            Style::new().green()
        } else {
            Style::new().blue()
        };

        let sender_name = state
            .get_user_name(message.sender_id)
            .unwrap_or("Unknown user");

        let header = Line::styled(sender_name, header_style);

        // TODO: Cache line wrapping? It's expensive logic to run every rendering tick.
        let wrapped_contents: Vec<Line> = textwrap::wrap(&message.contents, max_width as usize)
            .into_iter()
            .map(Line::from)
            .collect();

        let content: Vec<_> = std::iter::once(header)
            .chain(wrapped_contents)
            .chain(std::iter::once(Line::raw(""))) // Add a line between each message
            .collect();

        ListItem::new(Text::from(content))
    }
}
