// --- PHÂN ĐOẠN: RADIO VỚI ID VÀ VALUE FORM ---

use crate::traits::{EventResult, FormValue, FormWidget};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

pub struct RadioWidget {
    pub id: String,
    pub label: String,
    pub options: Vec<String>,
    pub selected_index: usize,
    focused: bool,
}

impl RadioWidget {
    pub fn new(id: impl Into<String>, label: impl Into<String>, options: Vec<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            options,
            selected_index: 0,
            focused: false,
        }
    }
}

impl FormWidget for RadioWidget {
    fn id(&self) -> &str {
        &self.id
    }

    fn value(&self) -> FormValue {
        let opt_name = self.options.get(self.selected_index).cloned().unwrap_or_default();
        FormValue::Select(self.selected_index, opt_name)
    }

    fn render(&self, area: Rect, frame: &mut Frame) {
        // Theo DESIGN.md mục 3: Label là BOLD + Secondary, 2-space indent
        let mut spans = vec![
            Span::styled("  ", Style::default()),
            Span::styled(
                format!("{}: ", self.label),
                if self.focused {
                    Style::default().fg(crate::Theme::PRIMARY).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(crate::Theme::SECONDARY)
                },
            ),
        ];

        for (idx, opt) in self.options.iter().enumerate() {
            let is_selected = idx == self.selected_index;
            let (symbol, opt_color) = if is_selected {
                (crate::Icons::RADIO_ON, crate::Theme::ACCENT)
            } else {
                (crate::Icons::RADIO_OFF, crate::Theme::MUTED)
            };

            let opt_style = if is_selected && self.focused {
                Style::default().fg(opt_color).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(opt_color)
            };

            spans.push(Span::styled(format!("{} {}  ", symbol, opt), opt_style));
        }

        frame.render_widget(Paragraph::new(Line::from(spans)), area);
    }

    fn handle_event(&mut self, key: KeyEvent) -> EventResult {
        if key.kind != KeyEventKind::Press {
            return EventResult::Ignored;
        }

        match key.code {
            // Chỉ dùng Left/Right (hoặc h/l) để chuyển option Radio theo hàng ngang
            KeyCode::Left | KeyCode::Char('h') => {
                if self.selected_index > 0 {
                    self.selected_index -= 1;
                    EventResult::Consumed
                } else {
                    EventResult::Ignored
                }
            }
            KeyCode::Right | KeyCode::Char('l') => {
                if self.selected_index + 1 < self.options.len() {
                    self.selected_index += 1;
                    EventResult::Consumed
                } else {
                    EventResult::Ignored
                }
            }
            // Up/Down hoàn toàn bỏ qua để FormManager chuyển lên/xuống widget khác trong form
            _ => EventResult::Ignored,
        }
    }

    fn focus(&mut self) { self.focused = true; }
    fn blur(&mut self) { self.focused = false; }
    fn is_focused(&self) -> bool { self.focused }
    fn preferred_height(&self) -> u16 { 2 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_radio_horizontal_selection_and_up_down_ignored() {
        let mut radio = RadioWidget::new(
            "env",
            "Environment",
            vec!["Prod".into(), "Staging".into(), "Dev".into()],
        );

        assert_eq!(radio.selected_index, 0);

        // Right -> chuyển sang Staging
        let right_ev = KeyEvent::new(KeyCode::Right, crossterm::event::KeyModifiers::NONE);
        assert_eq!(radio.handle_event(right_ev), EventResult::Consumed);
        assert_eq!(radio.selected_index, 1);

        // Up & Down -> phải bị Ignored để FormManager di chuyển widget
        let down_ev = KeyEvent::new(KeyCode::Down, crossterm::event::KeyModifiers::NONE);
        assert_eq!(radio.handle_event(down_ev), EventResult::Ignored);

        let up_ev = KeyEvent::new(KeyCode::Up, crossterm::event::KeyModifiers::NONE);
        assert_eq!(radio.handle_event(up_ev), EventResult::Ignored);

        // Left -> chuyển lùi về Prod
        let left_ev = KeyEvent::new(KeyCode::Left, crossterm::event::KeyModifiers::NONE);
        assert_eq!(radio.handle_event(left_ev), EventResult::Consumed);
        assert_eq!(radio.selected_index, 0);
    }
}