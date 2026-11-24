mod channel_list;
mod connection_status;
mod user_list;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    widgets::{Block, Borders, Widget},
};

use channel_list::ChannelList;
use connection_status::ConnectionStatus;
use user_list::UserList;

use crate::{
    connection_state::ConnectionState,
    ui::{Action, KeyHandler},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Unfocused,
    Channels,
    Users,
}

#[derive(Debug)]
pub struct Sidebar {
    focus: Focus,
    connection_status: ConnectionStatus,
    channel_list: ChannelList,
    user_list: UserList,
}

impl Sidebar {
    pub fn new() -> Self {
        Self {
            focus: Focus::Unfocused,
            connection_status: ConnectionStatus::new(),
            channel_list: ChannelList::new(Borders::TOP),
            user_list: UserList::new(Borders::TOP),
        }
    }

    pub fn render(&mut self, area: Rect, buf: &mut Buffer, state: Option<&ConnectionState>) {
        let outer_block = Block::bordered();
        let inner_area = outer_block.inner(area);

        outer_block.render(area, buf);

        let [connection_area, channels_area, users_area] = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![
                Constraint::Percentage(10),
                Constraint::Percentage(30),
                Constraint::Percentage(60),
            ])
            .areas(inner_area);

        self.connection_status.render(connection_area, buf, state);
        self.channel_list
            .render(channels_area, buf, state, self.focus == Focus::Channels);
        self.user_list
            .render(users_area, buf, state, self.focus == Focus::Users);
    }
}

impl KeyHandler for Sidebar {
    fn handle_key(&mut self, key: KeyEvent) -> Action {
        match self.focus {
            Focus::Unfocused => match key.code {
                KeyCode::Char('c') => {
                    self.focus = Focus::Channels;
                    Action::None
                }

                KeyCode::Char('u') => {
                    self.focus = Focus::Users;
                    Action::None
                }

                _ => unreachable!(
                    "If the sidebar is unfocused, the only keys the main panel will route here are handled above"
                ),
            },

            Focus::Channels => match key.code {
                KeyCode::Esc => {
                    self.focus = Focus::Unfocused;
                    Action::YieldFocus
                }

                KeyCode::Char('k') | KeyCode::Up => {
                    self.channel_list.scroll_up();
                    Action::None
                }

                KeyCode::Char('j') | KeyCode::Down => {
                    self.channel_list.scroll_down();
                    Action::None
                }

                KeyCode::Enter => {
                    let Some(id) = self.channel_list.select() else {
                        return Action::None;
                    };

                    Action::SelectChannel(id)
                }

                _ => Action::None,
            },

            Focus::Users => match key.code {
                KeyCode::Esc => {
                    self.focus = Focus::Unfocused;
                    Action::YieldFocus
                }

                KeyCode::Char('k') | KeyCode::Up => {
                    self.user_list.scroll_up();
                    Action::None
                }

                KeyCode::Char('j') | KeyCode::Down => {
                    self.user_list.scroll_down();
                    Action::None
                }

                KeyCode::Enter => {
                    let Some(id) = self.user_list.select() else {
                        return Action::None;
                    };

                    Action::SelectUser(id)
                }

                _ => Action::None,
            },
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
    fn test_focus_transitions_and_yielding() {
        let mut sidebar = Sidebar::new();
        assert_eq!(sidebar.focus, Focus::Unfocused);

        // Transition to Channels
        let action = sidebar.handle_key(press(KeyCode::Char('c')));
        assert!(matches!(action, Action::None));
        assert_eq!(sidebar.focus, Focus::Channels);

        // Escape yields focus back to the parent
        let action = sidebar.handle_key(press(KeyCode::Esc));
        assert!(matches!(action, Action::YieldFocus));
        assert_eq!(sidebar.focus, Focus::Unfocused);

        // Transition to Users
        let action = sidebar.handle_key(press(KeyCode::Char('u')));
        assert!(matches!(action, Action::None));
        assert_eq!(sidebar.focus, Focus::Users);

        // Escape yields focus back again
        let action = sidebar.handle_key(press(KeyCode::Esc));
        assert!(matches!(action, Action::YieldFocus));
    }

    #[test]
    #[should_panic(expected = "If the sidebar is unfocused")]
    fn test_unfocused_sidebar_panics_on_invalid_input() {
        let mut sidebar = Sidebar::new();

        // If the MainPanel routes a random key (like 'j') to the sidebar
        // while it is Unfocused, the sidebar should panic to flag the routing bug.
        sidebar.handle_key(press(KeyCode::Char('j')));
    }
}
