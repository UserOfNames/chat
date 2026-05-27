mod messages;
mod sidebar;

use chat_backend::ui_server_state::UIServerState;
use crossterm::event::{KeyCode, KeyEvent};

use super::{Action, KeyHandler, popups::commands::CommandsPopup};
use messages::Messages;
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    widgets::{Block, Widget},
};
use sidebar::Sidebar;
use tui_textarea::TextArea;

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
        Self {
            focus: Focus::None,
            input: Self::new_textbox(),
            messages: Messages::new(),
            sidebar: Sidebar::new(),
        }
    }

    /// Return a new textbox with default settings for `self.input`.
    fn new_textbox() -> TextArea<'static> {
        let block = Block::bordered().title(" Input ");
        let mut textbox = TextArea::default();
        textbox.set_block(block);
        textbox
    }

    /// Reset the input area.
    fn reset_input(&mut self) {
        self.input = Self::new_textbox();
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

        self.sidebar.render(sidebar, buf, state);
        self.messages.render(messages, buf, state);
        self.input.render(input, buf);
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
