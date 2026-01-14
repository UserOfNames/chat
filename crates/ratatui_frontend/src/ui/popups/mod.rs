pub mod commands;
pub mod connect;
pub mod notice;
pub mod quit;

use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Flex, Layout, Rect},
};

use super::{Action, KeyHandler};

pub enum SizeKind {
    Percentage(u16),
    Exact(u16),
}

pub type SizeHint = (SizeKind, SizeKind);

pub trait Popup: KeyHandler + std::fmt::Debug {
    fn render(&self, area: Rect, buf: &mut Buffer);
    fn hint_size(&self) -> SizeHint;
}

// https://ratatui.rs/examples/apps/popup/
pub fn popup_area(area: Rect, size: SizeHint) -> Rect {
    let (x, y) = size;

    let x_constraint = match x {
        SizeKind::Percentage(x) => Constraint::Percentage(x),
        SizeKind::Exact(x) => Constraint::Length(x),
    };

    let y_constraint = match y {
        SizeKind::Percentage(y) => Constraint::Percentage(y),
        SizeKind::Exact(y) => Constraint::Length(y),
    };

    let vertical = Layout::vertical([y_constraint]).flex(Flex::Center);
    let horizontal = Layout::horizontal([x_constraint]).flex(Flex::Center);
    let [area] = vertical.areas(area);
    let [area] = horizontal.areas(area);
    area
}
