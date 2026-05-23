// TODO: Rework in light of channel implementation
use std::cell::RefCell;

use ratatui::{
    prelude::{Buffer, Rect},
    widgets::{Block, List, ListItem, ListState, StatefulWidget, Widget},
};

use chat_backend::ui_server_state::UIServerState;

#[derive(Debug)]
pub struct Messages {
    messages: Vec<ListItem<'static>>,
    list_state: RefCell<ListState>,
}

impl Messages {
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
            list_state: RefCell::new(ListState::default()),
        }
    }

    // pub fn add_message(&mut self, message: ReceivedMessage) {
    //     // TODO: Align right if you're the sender
    //     let alignment = Alignment::Left;
    //
    //     let header = Line::from(vec![Span::styled(
    //         format!("{}: ", message.sender_id),
    //         Style::default().blue(),
    //     )])
    //     .alignment(alignment);
    //
    //     let content = format!("{}: {}", message.sender_id, message.contents);
    //     self.messages.push(ListItem::new(content));
    //     self.list_state
    //         .borrow_mut()
    //         .select(Some(self.messages.len() - 1));
    // }

    pub fn render(&mut self, area: Rect, buf: &mut Buffer, state: Option<&UIServerState>) {
        // TODO: Text wrapping?

        let block = Block::bordered().title(" Messages ");
        let inner_area = block.inner(area);
        block.render(area, buf);

        if let Some(state) = state
            // TODO: handle None context
            && let Some(messages) = state.messages.get(&state.message_context.clone().unwrap())
        {
            let list = List::new(messages.iter().map(|message| message.contents.as_str()))
                .highlight_symbol(">> ")
                .repeat_highlight_symbol(true);

            let mut state = self.list_state.borrow_mut();
            StatefulWidget::render(list, inner_area, buf, &mut state);
        }
    }
}
