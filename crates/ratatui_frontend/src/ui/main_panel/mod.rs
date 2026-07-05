mod messages;
mod sidebar;

use crossterm::event::{KeyCode, KeyEvent};

use super::{Action, KeyHandler, popups::commands::CommandsPopup};
use messages::Messages;
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    widgets::{Block, Widget},
};
use ratatui_textarea::TextArea;
use sidebar::Sidebar;

use crate::ui_server_state::UIServerState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    None,
    Input,
    Sidebar,
}

/// The main panel, consisting of an input box, a scrollable list of messages, and a sidebar
/// listing the connection state, channels, and users.
#[derive(Debug)]
pub struct MainPanel {
    focus: Focus,
    input: TextArea<'static>,
    messages: Messages,
    sidebar: Sidebar,
}

impl MainPanel {
    /// Create a new `MainPanel`.
    pub fn new() -> Self {
        let block = Block::bordered().title(" Input ");
        let mut input = TextArea::default();
        input.set_block(block);

        Self {
            focus: Focus::None,
            input,
            messages: Messages::new(),
            sidebar: Sidebar::new(),
        }
    }

    /// Reset the input area.
    fn reset_input(&mut self) {
        self.input.clear();
    }

    pub fn render(&mut self, area: Rect, buf: &mut Buffer, state: Option<&UIServerState>) {
        let [message_part, sidebar] = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(vec![Constraint::Percentage(80), Constraint::Percentage(20)])
            .areas(area);

        let [messages, input] = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![Constraint::Percentage(75), Constraint::Percentage(25)])
            .areas(message_part);

        self.set_widget_styles();

        self.sidebar.render(sidebar, buf, state);
        self.messages.render(messages, buf, state);
        self.input.render(input, buf);
    }

    /// Helper to set the styles of widgets owned by the `MainPanel` based on the current
    /// application state.
    fn set_widget_styles(&mut self) {
        let border_style = if self.focus == Focus::Input {
            Style::default().green()
        } else {
            Style::default()
        };
        self.input.set_block(
            Block::bordered()
                .title(" Input ")
                .border_style(border_style),
        );

        let cursor_style = if self.focus == Focus::Input {
            Style::default().reversed()
        } else {
            Style::default().hidden()
        };
        self.input.set_cursor_style(cursor_style);

        self.input.set_cursor_line_style(Style::default());
    }
}

impl KeyHandler for MainPanel {
    fn handle_key(&mut self, key: KeyEvent) -> super::Action {
        match self.focus {
            Focus::None => match key.code {
                KeyCode::Char('i') => {
                    self.focus = Focus::Input;
                    Action::None
                }

                KeyCode::Char('c') | KeyCode::Char('u') => {
                    self.focus = Focus::Sidebar;
                    self.sidebar.handle_key(key);
                    Action::None
                }

                KeyCode::Esc => Action::PushPopup(CommandsPopup::create()),

                KeyCode::Backspace => panic!("DEBUG remove this key"),

                _ => Action::None,
            },

            Focus::Input => match key.code {
                KeyCode::Esc => {
                    self.focus = Focus::None;
                    Action::None
                }

                KeyCode::Enter => {
                    let message = self.input.lines().join("");
                    self.reset_input();
                    Action::SendMessage(message)
                }

                _ => {
                    self.input.input(key);
                    Action::None
                }
            },

            Focus::Sidebar => {
                let action = self.sidebar.handle_key(key);
                if let Action::YieldFocus = action {
                    self.focus = Focus::None;
                }

                action
            }
        }
    }
}
