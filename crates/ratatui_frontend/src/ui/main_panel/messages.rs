use std::cell::RefCell;

use ratatui::{
    layout::Alignment,
    prelude::{Buffer, Rect},
    style::{Style, Stylize},
    text::{Line, Span},
    widgets::{Block, List, ListItem, ListState, StatefulWidget, Widget},
};

use chat_backend::client_event::ChatMessage;

#[derive(Debug)]
pub struct Messages {
    messages: Vec<ListItem<'static>>,
    state: RefCell<ListState>,
}

impl Messages {
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
            state: RefCell::new(ListState::default()),
        }
    }

    pub fn add_message(&mut self, message: ChatMessage) {
        // TODO: Align right if you're the sender
        let alignment = Alignment::Left;

        let header = Line::from(vec![Span::styled(
            format!("{}: ", message.sender),
            Style::default().blue(),
        )])
        .alignment(alignment);

        let content = format!("{}: {}", message.sender, message.contents);
        self.messages.push(ListItem::new(content));
        self.state
            .borrow_mut()
            .select(Some(self.messages.len() - 1));
    }
}

impl Widget for &Messages {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // TODO: Text wrapping

        let block = Block::bordered().title(" Messages ");
        let inner_area = block.inner(area);
        block.render(area, buf);

        let list = List::new(self.messages.clone())
            .highlight_symbol(">> ")
            .repeat_highlight_symbol(true);

        let mut state = self.state.borrow_mut();
        StatefulWidget::render(list, inner_area, buf, &mut state);
    }
}
