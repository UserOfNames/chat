use chat_backend::client_command::ConnectParams;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Widget},
};
use ratatui_textarea::TextArea;

use super::{
    Action, KeyHandler, Popup, SizeHint, SizeKind,
    notice::{NoticeLevel, NoticePopup},
};

const FIELD_COUNT: usize = 3;

#[repr(usize)]
#[derive(Debug, Clone, Copy)]
enum Focus {
    Host = 0,
    Username = 1,
    Port = 2,
}

impl TryFrom<usize> for Focus {
    type Error = ();

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Focus::Host),
            1 => Ok(Focus::Username),
            2 => Ok(Focus::Port),
            _ => Err(()),
        }
    }
}

impl Focus {
    fn prev(&self) -> Self {
        (((*self as usize) + FIELD_COUNT - 1) % FIELD_COUNT)
            .try_into()
            .expect("Integer range is bounded by variant count, so this can never trigger")
    }

    fn next(&self) -> Self {
        (((*self as usize) + 1) % FIELD_COUNT)
            .try_into()
            .expect("Integer range is bounded by variant count, so this can never trigger")
    }
}

#[derive(Debug)]
pub struct ConnectPopup {
    inputs: [TextArea<'static>; FIELD_COUNT],
    focus: Focus,
}

impl ConnectPopup {
    pub fn create() -> Box<dyn Popup> {
        let mut inputs = [
            TextArea::default(),
            TextArea::default(),
            TextArea::default(),
        ];

        inputs[Focus::Host as usize].set_placeholder_text("Host (IP or Domain)");
        inputs[Focus::Username as usize].set_placeholder_text("Username");
        inputs[Focus::Port as usize].set_placeholder_text("Port (optional)");

        let mut popup = Self {
            inputs,
            focus: Focus::Host,
        };

        popup.apply_focus_styles();

        Box::new(popup)
    }

    // Helper function to update the block state of each field according to the current Focus.
    fn apply_focus_styles(&mut self) {
        for (i, input) in self.inputs.iter_mut().enumerate() {
            let is_focused = i == self.focus as usize;

            let border_style = if is_focused {
                Style::default().green()
            } else {
                Style::default()
            };

            let block = Block::default()
                .borders(Borders::TOP)
                .border_style(border_style);

            let cursor_style = if is_focused {
                Style::default().add_modifier(Modifier::REVERSED)
            } else {
                Style::default()
            };

            input.set_block(block);
            input.set_cursor_style(cursor_style);
        }
    }
}

impl KeyHandler for ConnectPopup {
    fn handle_key(&mut self, key: KeyEvent) -> Action {
        match key.code {
            KeyCode::Esc => Action::PopPopup,

            KeyCode::Tab | KeyCode::Down => {
                self.focus = self.focus.next();
                self.apply_focus_styles();
                Action::None
            }

            KeyCode::BackTab | KeyCode::Up => {
                self.focus = self.focus.prev();
                self.apply_focus_styles();
                Action::None
            }

            KeyCode::Enter => {
                // TODO: fast trim?
                let username = self.inputs[Focus::Username as usize]
                    .lines()
                    .join("")
                    .trim()
                    .to_owned();

                // TODO: fast trim?
                let host = self.inputs[Focus::Host as usize]
                    .lines()
                    .join("")
                    .trim()
                    .to_owned();

                // TODO: fast trim?
                let port_raw = self.inputs[Focus::Port as usize]
                    .lines()
                    .join("")
                    .trim()
                    .to_owned();

                if username.is_empty() {
                    return Action::PushPopup(NoticePopup::create(
                        "Must enter a username".to_owned(),
                        NoticeLevel::Error,
                    ));
                }

                let port = if port_raw.is_empty() {
                    None
                } else {
                    match port_raw.parse::<u16>() {
                        Ok(p) => Some(p),
                        Err(e) => {
                            return Action::PushPopup(NoticePopup::create(
                                format!("Could not parse the port '{port_raw}': {e}"),
                                NoticeLevel::Error,
                            ));
                        }
                    }
                };

                let params = ConnectParams {
                    host,
                    port,
                    initial_username: username,
                };

                Action::Connect(params)
            }

            _ => {
                self.inputs[self.focus as usize].input(key);
                Action::None
            }
        }
    }
}

impl Popup for ConnectPopup {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let outer_block = Block::bordered();
        let inner_area = outer_block.inner(area);
        outer_block.render(area, buf);

        let areas: [_; FIELD_COUNT + 1] = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Length(2),
                Constraint::Length(2),
                Constraint::Length(2),
            ])
            .areas(inner_area);

        let help_line = Line::from_iter([
            Span::styled("Connect: Enter", Style::default().blue()),
            Span::raw(" • "),
            Span::styled("Next: Tab/↓", Style::default().blue()),
            Span::raw(" • "),
            Span::styled("Prev: ↑", Style::default().blue()),
        ]).alignment(Alignment::Center);

        help_line.render(areas[0], buf);

        for (i, input) in self.inputs.iter().enumerate() {
            // The first area is for the text header, so we need to offset by 1
            input.render(areas[i + 1], buf);
        }
    }

    fn hint_size(&self) -> SizeHint {
        (SizeKind::Percentage(50), SizeKind::Exact(9))
    }
}
