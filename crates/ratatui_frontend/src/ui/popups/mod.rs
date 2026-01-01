pub mod commands;
pub mod connect;
pub mod notice;
pub mod quit;

use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Flex, Layout, Rect},
};

use super::{Action, KeyHandler};

pub trait Popup: KeyHandler + std::fmt::Debug {
    fn render(&self, area: Rect, buf: &mut Buffer);
    fn hint_size(&self) -> (u16, u16);
}

// https://ratatui.rs/examples/apps/popup/
pub fn popup_area(area: Rect, percent_x: u16, percent_y: u16) -> Rect {
    let vertical = Layout::vertical([Constraint::Percentage(percent_y)]).flex(Flex::Center);
    let horizontal = Layout::horizontal([Constraint::Percentage(percent_x)]).flex(Flex::Center);
    let [area] = vertical.areas(area);
    let [area] = horizontal.areas(area);
    area
}
