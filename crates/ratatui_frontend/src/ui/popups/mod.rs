pub mod commands;
pub mod connect;
pub mod notice;
pub mod quit;
pub mod update_info;

use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Rect},
};

use super::{Action, KeyHandler};

pub type SizeHint = (Constraint, Constraint);

pub trait Popup: KeyHandler + std::fmt::Debug {
    fn render(&self, area: Rect, buf: &mut Buffer);
    fn hint_size(&self) -> SizeHint;
}
