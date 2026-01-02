use ratatui::{
    layout::{Constraint, Direction, Layout},
    prelude::{Buffer, Rect},
    widgets::{Block, Borders, Paragraph, Widget},
};

#[derive(Debug)]
pub struct Sidebar {
    connected_addr: Option<String>,
}

impl Sidebar {
    pub fn new() -> Self {
        Self {
            connected_addr: None,
        }
    }
}

impl Widget for &Sidebar {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let outer_block = Block::bordered();
        let inner_area = outer_block.inner(area);

        outer_block.render(area, buf);

        let [connection, channels, users] = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![
                Constraint::Percentage(10),
                Constraint::Percentage(80),
                Constraint::Percentage(10),
            ])
            .areas(inner_area);

        Paragraph::new("connection info").render(connection, buf);

        Paragraph::new("channel list")
            .block(Block::default().borders(Borders::TOP))
            .render(channels, buf);

        Paragraph::new("user list")
            .block(Block::default().borders(Borders::TOP))
            .render(users, buf);
    }
}
