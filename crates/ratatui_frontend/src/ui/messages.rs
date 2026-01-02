use ratatui::{prelude::{Buffer, Rect}, widgets::{Block, Widget}};

#[derive(Debug)]
pub struct Messages;

impl Messages {
    pub fn new() -> Self {
        Self
    }
}

impl Widget for &Messages {
    fn render(self, area: Rect, buf: &mut Buffer) {
        Block::bordered().title(" Messages ").render(area, buf);
    }
}
