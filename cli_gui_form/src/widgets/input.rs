// --- PHÂN ĐOẠN: TEXT INPUT VỚI CON TRỎ TERMINAL VÀ CHỈNH SỬA TẠI CHỖ ---

use crate::traits::{EventResult, FormValue, FormWidget};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    widgets::{Block, BorderType, Borders, Padding, Paragraph},
    Frame,
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Text,
    Password,
}

pub struct InputWidget {
    pub id: String,
    pub label: String,
    pub value: String,
    pub placeholder: String,
    pub mode: InputMode,
    cursor_idx: usize,
    focused: bool,
}

impl InputWidget {
    /****
     * Function: new
     * Chức năng: Khởi tạo ô nhập liệu với id, nhãn và chế độ hiển thị (Text/Password).
     * Ranh giới bảo vệ: Giá trị ban đầu rỗng, con trỏ tại vị trí 0, chưa kích hoạt focus.
     ****/
    pub fn new(id: impl Into<String>, label: impl Into<String>, mode: InputMode) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            value: String::new(),
            placeholder: String::new(),
            mode,
            cursor_idx: 0,
            focused: false,
        }
    }

    /****
     * Function: with_placeholder
     * Chức năng: Thiết lập chuỗi văn bản gợi ý khi ô chưa được điền thông tin.
     * Ranh giới bảo vệ: Hiển thị mờ bằng Theme::NEUTRAL_300 (#666666) theo chuẩn DESIGN.md.
     ****/
    pub fn with_placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /****
     * Function: with_value
     * Chức năng: Gán giá trị ban đầu và đưa con trỏ về cuối chuỗi.
     ****/
    pub fn with_value(mut self, value: impl Into<String>) -> Self {
        self.value = value.into();
        self.cursor_idx = self.value.len();
        self
    }

    pub fn placeholder(&self) -> &str {
        &self.placeholder
    }

    pub fn set_placeholder(&mut self, placeholder: impl Into<String>) {
        self.placeholder = placeholder.into();
    }
}

impl FormWidget for InputWidget {
    fn id(&self) -> &str {
        &self.id
    }

    fn value(&self) -> FormValue {
        FormValue::Text(self.value.clone())
    }

    fn render(&self, area: Rect, frame: &mut Frame) {
        // Theo chuẩn DESIGN.md:
        // - Khi ô rỗng và có placeholder: hiển thị placeholder với Theme::NEUTRAL_300 (#666666)
        // - Khi đã có dữ liệu: hiển thị text hoặc ký tự '*' nếu Password, màu Primary BOLD khi focus
        let (display_text, text_style) = if self.value.is_empty() && !self.placeholder.is_empty() {
            (
                self.placeholder.clone(),
                Style::default().fg(crate::Theme::NEUTRAL_300),
            )
        } else {
            let text = match self.mode {
                InputMode::Text => self.value.clone(),
                InputMode::Password => "*".repeat(self.value.len()),
            };
            let style = if self.focused {
                Style::default().fg(crate::Theme::PRIMARY).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(crate::Theme::FG)
            };
            (text, style)
        };

        // - Active/Focused: BORDER_FOCUS (#0070f3)
        // - Inactive: Muted color border (#555555 / Neutral 100)
        let border_color = if self.focused {
            crate::Theme::BORDER_FOCUS
        } else {
            crate::Theme::MUTED
        };

        let title_formatted = format!("─ {} ─", self.label);

        // Bo góc theo chuẩn Minimal (BorderType::Rounded) kết hợp format ╭─ Label ─
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(border_color))
            .padding(Padding::horizontal(1))
            .title(title_formatted);

        let paragraph = Paragraph::new(display_text)
            .block(block)
            .style(text_style);

        frame.render_widget(paragraph, area);
    }

    fn handle_event(&mut self, key: KeyEvent) -> EventResult {
        if key.kind != KeyEventKind::Press {
            return EventResult::Ignored;
        }

        match key.code {
            KeyCode::Left => {
                if self.cursor_idx > 0 {
                    self.cursor_idx -= 1;
                    EventResult::Consumed
                } else {
                    EventResult::Ignored
                }
            }
            KeyCode::Right => {
                if self.cursor_idx < self.value.len() {
                    self.cursor_idx += 1;
                    EventResult::Consumed
                } else {
                    EventResult::Ignored
                }
            }
            KeyCode::Home => {
                self.cursor_idx = 0;
                EventResult::Consumed
            }
            KeyCode::End => {
                self.cursor_idx = self.value.len();
                EventResult::Consumed
            }
            KeyCode::Backspace => {
                if self.cursor_idx > 0 {
                    self.cursor_idx -= 1;
                    self.value.remove(self.cursor_idx);
                    EventResult::Consumed
                } else {
                    EventResult::Ignored
                }
            }
            KeyCode::Char(c) => {
                self.value.insert(self.cursor_idx, c);
                self.cursor_idx += 1;
                EventResult::Consumed
            }
            _ => EventResult::Ignored,
        }
    }

    fn focus(&mut self) {
        self.focused = true;
    }

    fn blur(&mut self) {
        self.focused = false;
    }

    fn is_focused(&self) -> bool {
        self.focused
    }

    fn preferred_height(&self) -> u16 {
        3
    }

    fn cursor_position(&self, area: Rect) -> Option<(u16, u16)> {
        if self.focused {
            // Tính toán vị trí con trỏ: area.x + 1 (border) + 1 (padding) + cursor_idx
            let x = area.x + 2 + (self.cursor_idx as u16);
            let y = area.y + 1; // Hàng thứ 2 (bên trong block border)
            Some((x, y))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_placeholder_builder() {
        let input = InputWidget::new("host", "Hostname", InputMode::Text)
            .with_placeholder("127.0.0.1:8080");

        assert_eq!(input.placeholder(), "127.0.0.1:8080");
        assert_eq!(input.value(), FormValue::Text(String::new()));
    }

    #[test]
    fn test_input_typing_and_backspace() {
        let mut input = InputWidget::new("db", "Database", InputMode::Text)
            .with_placeholder("postgres");

        assert_eq!(input.value, "");

        // Nhập 'a', 'b', 'c'
        let _ = input.handle_event(KeyEvent::from(KeyCode::Char('a')));
        let _ = input.handle_event(KeyEvent::from(KeyCode::Char('b')));
        let _ = input.handle_event(KeyEvent::from(KeyCode::Char('c')));
        assert_eq!(input.value, "abc");
        assert_eq!(input.value(), FormValue::Text("abc".to_string()));

        // Backspace
        let _ = input.handle_event(KeyEvent::from(KeyCode::Backspace));
        assert_eq!(input.value, "ab");

        // Cursor Left và nhập chèn ở giữa
        let _ = input.handle_event(KeyEvent::from(KeyCode::Left));
        let _ = input.handle_event(KeyEvent::from(KeyCode::Char('x')));
        assert_eq!(input.value, "axb");
    }
}