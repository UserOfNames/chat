use std::borrow::Cow;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Rect},
    style::Style,
    text::Text,
    widgets::{Block, Paragraph, Widget, Wrap},
};

use super::{Action, KeyHandler, Popup, SizeHint};

#[derive(Debug)]
pub enum NoticeLevel {
    Notification,
    Warning,
    Error,
}

#[derive(Debug)]
pub struct NoticePopup {
    message: Cow<'static, str>,
    level: NoticeLevel,
    scroll: u16,
}

impl NoticePopup {
    const X_PADDING: u16 = 2; // Borders
    const Y_PADDING: u16 = 2; // Borders
    const MAX_WIDTH: u16 = 80 + Self::X_PADDING;
    const MIN_WIDTH: u16 = 30 + Self::X_PADDING;
    const MIN_HEIGHT: u16 = 5 + Self::Y_PADDING;
    const MAX_HEIGHT: u16 = 20 + Self::Y_PADDING;

    pub fn create(message: impl Into<Cow<'static, str>>, level: NoticeLevel) -> Box<dyn Popup> {
        Box::new(Self {
            message: message.into(),
            level,
            scroll: 0,
        })
    }

    /// Helper to calculate the width of the popup and total number of wrapped lines (basis for the
    /// height of the popup). The total lines alone is likely not the final height of the popup. To
    /// compute that, pass the total line count to `Self::final_height`.
    ///
    /// Returns `(target_width, total_lines)`.
    // TODO: This abstraction is brittle. Find a better way.
    #[allow(clippy::cast_possible_truncation)]
    fn content_dimensions(&self) -> (u16, u16) {
        let max_line_len = self
            .message
            .lines()
            .map(|line| line.chars().count())
            .max()
            .unwrap_or(0) as u16;

        // Total popup width, including borders
        let target_width = max_line_len
            .saturating_add(Self::X_PADDING)
            .clamp(Self::MIN_WIDTH, Self::MAX_WIDTH);

        // Inner width, excluding borders
        let inner_width = target_width.saturating_sub(Self::X_PADDING);

        let mut total_lines: u16 = 0;
        for line in self.message.lines() {
            let line_len = line.chars().count() as u16;
            let wrapped_lines = line_len.div_ceil(inner_width.max(1));
            // `.max(1)` is needed so that empty lines still display
            total_lines += wrapped_lines.max(1);
        }

        (target_width, total_lines)
    }

    /// Helper to compute the height of the popup, given the wrapped line count.
    fn target_height(total_lines: u16) -> u16 {
        total_lines
            .saturating_add(Self::Y_PADDING)
            .clamp(Self::MIN_HEIGHT, Self::MAX_HEIGHT)
    }
}

impl KeyHandler for NoticePopup {
    fn handle_key(&mut self, key: KeyEvent) -> Action {
        match key.code {
            KeyCode::Esc => Action::PopPopup,

            KeyCode::Up | KeyCode::Char('k') => {
                self.scroll = self.scroll.saturating_sub(1);
                Action::None
            }

            KeyCode::Down | KeyCode::Char('j') => {
                let (_, total_lines) = self.content_dimensions();
                let target_height = Self::target_height(total_lines);
                let visible_height = target_height.saturating_sub(Self::Y_PADDING);

                let max_scroll = total_lines.saturating_sub(visible_height);

                self.scroll = self.scroll.saturating_add(1).min(max_scroll);
                Action::None
            }

            _ => Action::None,
        }
    }
}

impl Popup for NoticePopup {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let (border_title, border_style) = match self.level {
            NoticeLevel::Notification => (" Notification ", Style::default().green()),
            NoticeLevel::Warning => (" Warning ", Style::default().yellow()),
            NoticeLevel::Error => (" Error ", Style::default().red()),
        };

        let block = Block::bordered()
            .title(border_title)
            .title_alignment(Alignment::Center)
            .border_style(border_style);

        let text = Text::from(self.message.as_ref());

        Paragraph::new(text)
            .block(block)
            .wrap(Wrap { trim: true })
            .scroll((self.scroll, 0))
            .render(area, buf);
    }

    fn hint_size(&self) -> SizeHint {
        let (target_width, total_lines) = self.content_dimensions();
        let target_height = Self::target_height(total_lines);

        (
            Constraint::Length(target_width),
            Constraint::Length(target_height),
        )
    }
}

#[cfg(test)]
mod test {
    use crate::ui::popups::connect::ConnectPopup;

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
    fn test_notice_popup_key_handling() {
        let mut popup = ConnectPopup::create();

        // Esc triggers PopPopup
        assert!(matches!(
            popup.handle_key(press(KeyCode::Esc)),
            Action::PopPopup
        ));

        // Up or 'k' scrolls up, triggering None
        assert!(matches!(popup.handle_key(press(KeyCode::Up)), Action::None));
        assert!(matches!(
            popup.handle_key(press(KeyCode::Char('k'))),
            Action::None
        ));

        // Down or 'j' scrolls down, triggering None
        assert!(matches!(
            popup.handle_key(press(KeyCode::Down)),
            Action::None
        ));
        assert!(matches!(
            popup.handle_key(press(KeyCode::Char('j'))),
            Action::None
        ));
    }

    #[test]
    fn test_dimensions_clamp_to_minimums() {
        let popup = NoticePopup {
            message: Cow::Borrowed("Short"),
            level: NoticeLevel::Notification,
            scroll: 0,
        };

        let (width, lines) = popup.content_dimensions();
        assert_eq!(width, NoticePopup::MIN_WIDTH);
        assert_eq!(lines, 1);

        let height = NoticePopup::target_height(lines);
        assert_eq!(height, NoticePopup::MIN_HEIGHT);
    }

    #[test]
    fn test_dimensions_clamp_to_maximums_and_wrap() {
        let long_message = "A".repeat(2000);
        let popup = NoticePopup {
            message: Cow::Owned(long_message),
            level: NoticeLevel::Error,
            scroll: 0,
        };

        let (width, lines) = popup.content_dimensions();
        assert_eq!(width, NoticePopup::MAX_WIDTH);

        // Inner width is 80. 200 chars / 80 = 25 -> ceil to 25 lines.
        assert_eq!(lines, 25);

        let height = NoticePopup::target_height(lines);
        // 25 lines should be clamped to the MAX_HEIGHT
        assert_eq!(height, NoticePopup::MAX_HEIGHT);
    }

    #[test]
    fn test_scrolling_clamps_correctly() {
        // Create 30 lines of text (exceeds MAX_HEIGHT of 22)
        let tall_message = "Line\n".repeat(30);
        let mut popup = NoticePopup {
            message: Cow::Owned(tall_message),
            level: NoticeLevel::Notification,
            scroll: 0,
        };

        // 30 lines. target_height = 22 (clamped). visible_height = 20.
        // max_scroll = 30 - 20 = 10.

        // Test scrolling down
        popup.handle_key(press(KeyCode::Char('j')));
        assert_eq!(popup.scroll, 1);

        popup.handle_key(press(KeyCode::Down));
        assert_eq!(popup.scroll, 2);

        // Scroll past the maximum
        for _ in 0..20 {
            popup.handle_key(press(KeyCode::Down));
        }
        assert_eq!(popup.scroll, 10, "Scroll should clamp at max_scroll (10)");

        // Test scrolling back up
        popup.handle_key(press(KeyCode::Char('k')));
        assert_eq!(popup.scroll, 9);

        // Scroll past the top
        for _ in 0..20 {
            popup.handle_key(press(KeyCode::Up));
        }
        assert_eq!(popup.scroll, 0, "Scroll should clamp at 0");
    }
}
