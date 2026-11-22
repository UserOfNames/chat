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

use crate::connection_state::ConnectionState;

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

    pub fn render(&mut self, area: Rect, buf: &mut Buffer, state: Option<&ConnectionState>) {
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

                KeyCode::Char('c' | 'u') => {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyEventKind, KeyModifiers};

    fn press(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::empty(),
            kind: KeyEventKind::Press,
            state: crossterm::event::KeyEventState::empty(),
        }
    }

    #[test]
    fn test_default_state_and_input_transitions() {
        let mut panel = MainPanel::new();
        assert_eq!(panel.focus, Focus::None);

        // Enter Input mode
        let action = panel.handle_key(press(KeyCode::Char('i')));
        assert_eq!(panel.focus, Focus::Input);
        assert!(matches!(action, Action::None));

        // Escape out of Input mode
        let action = panel.handle_key(press(KeyCode::Esc));
        assert_eq!(panel.focus, Focus::None);
        assert!(matches!(action, Action::None));
    }

    #[test]
    fn test_typing_and_sending_message() {
        let mut panel = MainPanel::new();

        // Enter input mode
        panel.handle_key(press(KeyCode::Char('i')));

        // Type "hi"
        panel.handle_key(press(KeyCode::Char('h')));
        panel.handle_key(press(KeyCode::Char('i')));

        // Verify the textarea captured it
        assert_eq!(panel.input.lines().join(""), "hi");

        // Press Enter
        let action = panel.handle_key(press(KeyCode::Enter));

        // Verify action is dispatched with the correct text
        match action {
            Action::SendMessage(msg) => assert_eq!(msg, "hi"),
            _ => panic!("Expected SendMessage, got {action:?}"),
        }

        // Verify the input box was cleared for the next message
        assert_eq!(panel.input.lines().join(""), "");
        assert_eq!(panel.focus, Focus::Input);
    }

    #[test]
    fn test_sidebar_delegation_and_yielding() {
        let mut panel = MainPanel::new();

        // 'c' should delegate to Sidebar and shift focus
        let action = panel.handle_key(press(KeyCode::Char('c')));
        assert_eq!(panel.focus, Focus::Sidebar);
        assert!(matches!(action, Action::None));

        // 'Esc' should be routed to the Sidebar, which returns YieldFocus.
        // The MainPanel must catch this and drop its focus to None.
        let action = panel.handle_key(press(KeyCode::Esc));
        assert_eq!(panel.focus, Focus::None);
        assert!(matches!(action, Action::YieldFocus));
    }

    #[test]
    fn test_global_commands_popup() {
        let mut panel = MainPanel::new();

        // When unfocused, Esc should open the command palette
        let action = panel.handle_key(press(KeyCode::Esc));

        assert!(matches!(action, Action::PushPopup(_)));
    }
}
