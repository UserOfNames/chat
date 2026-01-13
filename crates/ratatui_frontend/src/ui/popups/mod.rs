pub mod commands;
pub mod connect;
pub mod notice;
pub mod quit;

use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Flex, Layout, Rect},
};

use super::{Action, KeyHandler};

pub enum SizeHint {
    Percentage(u16, u16),
    Exact(u16, u16),
}

pub trait Popup: KeyHandler + std::fmt::Debug {
    fn render(&self, area: Rect, buf: &mut Buffer);
    fn hint_size(&self) -> SizeHint;
}

// https://ratatui.rs/examples/apps/popup/
pub fn popup_area(area: Rect, size: SizeHint) -> Rect {
    let (x_constraint, y_constraint) = match size {
        SizeHint::Percentage(x, y) => (Constraint::Percentage(x), Constraint::Percentage(y)),
        SizeHint::Exact(x, y) => (Constraint::Length(x), Constraint::Length(y)),
    };

    let vertical = Layout::vertical([y_constraint]).flex(Flex::Center);
    let horizontal = Layout::horizontal([x_constraint]).flex(Flex::Center);
    let [area] = vertical.areas(area);
    let [area] = horizontal.areas(area);
    area
}
