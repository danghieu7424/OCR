// --- PHÂN ĐOẠN: LIST / MENU WIDGET CHUẨN DESIGN.MD MỤC 5 VỚI SUB-LEVEL NAVIGATION ---

use crate::{
    theme::Theme,
    traits::{EventResult, FormValue, FormWidget},
};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

/****
 * Struct: ListItem
 * Chức năng: Đại diện cho một phần tử trong danh sách chọn/menu.
 * Ranh giới: Hỗ trợ trạng thái kích hoạt (Active) hoặc vô hiệu hóa (Disabled).
 ****/
#[derive(Clone, Debug, PartialEq)]
pub struct ListItem {
    pub text: String,
    pub disabled: bool,
}

impl ListItem {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            disabled: false,
        }
    }

    pub fn disabled(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            disabled: true,
        }
    }
}

/****
 * Struct: ListWidget
 * Chức năng: Điều hướng danh sách / menu dọc theo đặc tả DESIGN.md mục 5 (Lists / Menus).
 *            - Selected: `▸` prefix + BOLD + Primary
 *            - Normal: 4-space indent + Foreground
 *            - Disabled: 4-space indent + Muted + dim
 * Ranh giới bảo vệ:
 *            - Hỗ trợ cơ chế Nested Sub-level Navigation:
 *              + Khi Focus: Mặc định ở chế độ Passive (Up/Down nhường cho Form chuyển widget).
 *              + Nhấn Right (→) hoặc Enter: Đi sâu vào bên trong danh sách (Active mode), Up/Down chọn item.
 *              + Nhấn Left (←) hoặc Esc: Thoát khỏi danh sách về lại cấp độ Form.
 ****/
pub struct ListWidget {
    pub id: String,
    pub label: Option<String>,
    pub items: Vec<ListItem>,
    pub selected: usize,
    focused: bool,
    active: bool,
}

impl ListWidget {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: None,
            items: Vec::new(),
            selected: 0,
            focused: false,
            active: false,
        }
    }

    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn with_item(mut self, text: impl Into<String>) -> Self {
        self.items.push(ListItem::new(text));
        self
    }

    pub fn with_disabled_item(mut self, text: impl Into<String>) -> Self {
        self.items.push(ListItem::disabled(text));
        self
    }

    pub fn add_item(&mut self, text: impl Into<String>) -> &mut Self {
        self.items.push(ListItem::new(text));
        self
    }

    pub fn add_disabled_item(&mut self, text: impl Into<String>) -> &mut Self {
        self.items.push(ListItem::disabled(text));
        self
    }

    pub fn selected(&self) -> usize {
        self.selected
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn set_active(&mut self, active: bool) {
        self.active = active;
    }

    pub fn selected_item(&self) -> Option<&ListItem> {
        self.items.get(self.selected)
    }

    pub fn select_next(&mut self) {
        if self.items.is_empty() {
            return;
        }
        let total = self.items.len();
        let mut next = (self.selected + 1) % total;
        // Tìm item khả dụng tiếp theo
        for _ in 0..total {
            if !self.items[next].disabled {
                self.selected = next;
                return;
            }
            next = (next + 1) % total;
        }
    }

    pub fn select_prev(&mut self) {
        if self.items.is_empty() {
            return;
        }
        let total = self.items.len();
        let mut prev = if self.selected == 0 {
            total - 1
        } else {
            self.selected - 1
        };
        // Tìm item khả dụng phía trước
        for _ in 0..total {
            if !self.items[prev].disabled {
                self.selected = prev;
                return;
            }
            prev = if prev == 0 { total - 1 } else { prev - 1 };
        }
    }
}

impl FormWidget for ListWidget {
    fn id(&self) -> &str {
        &self.id
    }

    fn value(&self) -> FormValue {
        if let Some(item) = self.selected_item() {
            FormValue::Select(self.selected, item.text.clone())
        } else {
            FormValue::None
        }
    }

    fn render(&self, area: Rect, frame: &mut Frame) {
        let mut lines = Vec::new();

        // 1. Render nhãn tiêu đề cùng gợi ý phím điều hướng phân cấp
        if let Some(ref lbl) = self.label {
            let mut label_spans = Vec::new();
            if self.focused && self.active {
                label_spans.push(Span::styled(
                    format!("  {} ", lbl),
                    Style::default().fg(Theme::PRIMARY).add_modifier(Modifier::BOLD),
                ));
                label_spans.push(Span::styled(
                    "[Active: Use ↑/↓ to choose, ← to exit]",
                    Style::default().fg(Theme::ACCENT).add_modifier(Modifier::BOLD),
                ));
            } else if self.focused {
                label_spans.push(Span::styled(
                    format!("  {} ", lbl),
                    Style::default().fg(Theme::PRIMARY).add_modifier(Modifier::BOLD),
                ));
                label_spans.push(Span::styled(
                    "(Press → to enter list)",
                    Style::default().fg(Theme::MUTED),
                ));
            } else {
                label_spans.push(Span::styled(
                    format!("  {}", lbl),
                    Style::default().fg(Theme::SECONDARY).add_modifier(Modifier::BOLD),
                ));
            }
            lines.push(Line::from(label_spans));
        }

        // 2. Render các mục trong danh sách theo chuẩn DESIGN.md mục 5
        for (idx, item) in self.items.iter().enumerate() {
            let is_sel = idx == self.selected;

            let (prefix, text_style) = if item.disabled {
                // Disabled: 4-space indent + Muted + dim
                (
                    "    ",
                    Style::default()
                        .fg(Theme::NEUTRAL_200)
                        .add_modifier(Modifier::DIM),
                )
            } else if is_sel {
                // Selected: `▸` prefix + BOLD + Primary (Sáng rực khi Active)
                let color = if self.active {
                    Theme::PRIMARY
                } else if self.focused {
                    Theme::ACCENT
                } else {
                    Theme::SECONDARY
                };
                let style = Style::default().fg(color).add_modifier(Modifier::BOLD);
                ("  ▸ ", style)
            } else {
                // Normal: 4-space indent + Foreground
                ("    ", Style::default().fg(Theme::FG))
            };

            let prefix_span = if is_sel && !item.disabled {
                let prefix_color = if self.active {
                    Theme::PRIMARY
                } else if self.focused {
                    Theme::ACCENT
                } else {
                    Theme::SECONDARY
                };
                Span::styled(
                    prefix,
                    Style::default().fg(prefix_color).add_modifier(Modifier::BOLD),
                )
            } else {
                Span::raw(prefix)
            };

            lines.push(Line::from(vec![
                prefix_span,
                Span::styled(item.text.clone(), text_style),
            ]));
        }

        frame.render_widget(Paragraph::new(lines), area);
    }

    fn handle_event(&mut self, key: KeyEvent) -> EventResult {
        if key.kind != KeyEventKind::Press {
            return EventResult::Ignored;
        }

        if !self.active {
            // Khi chưa kích hoạt sâu vào trong list:
            match key.code {
                KeyCode::Right | KeyCode::Enter | KeyCode::Char(' ') | KeyCode::Char('l') => {
                    self.active = true;
                    EventResult::Consumed
                }
                // Nhường Up/Down cho FormManager chuyển widget khác
                _ => EventResult::Ignored,
            }
        } else {
            // Khi đang ở chế độ Active điều hướng các item con:
            match key.code {
                KeyCode::Down | KeyCode::Char('j') => {
                    self.select_next();
                    EventResult::Consumed
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    self.select_prev();
                    EventResult::Consumed
                }
                KeyCode::Home => {
                    for (idx, it) in self.items.iter().enumerate() {
                        if !it.disabled {
                            self.selected = idx;
                            break;
                        }
                    }
                    EventResult::Consumed
                }
                KeyCode::End => {
                    for (idx, it) in self.items.iter().enumerate().rev() {
                        if !it.disabled {
                            self.selected = idx;
                            break;
                        }
                    }
                    EventResult::Consumed
                }
                KeyCode::Left | KeyCode::Esc | KeyCode::Enter | KeyCode::Char('h') => {
                    self.active = false;
                    EventResult::Consumed
                }
                _ => EventResult::Ignored,
            }
        }
    }

    fn focus(&mut self) {
        self.focused = true;
        self.active = false;
    }

    fn blur(&mut self) {
        self.focused = false;
        self.active = false;
    }

    fn is_focused(&self) -> bool {
        self.focused
    }

    fn preferred_height(&self) -> u16 {
        (self.items.len() + if self.label.is_some() { 1 } else { 0 }) as u16
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_widget_sub_level_navigation() {
        let mut list = ListWidget::new("routes")
            .with_item("api/routes.ts")
            .with_disabled_item("api/handler.ts (locked)")
            .with_item("lib/utils.ts")
            .with_item("config.json");

        list.focus();
        assert!(list.is_focused());
        assert!(!list.is_active());

        // Khi chưa active, bấm Down phải bị Ignored để FormManager di chuyển
        let down_event = KeyEvent::new(KeyCode::Down, crossterm::event::KeyModifiers::NONE);
        assert_eq!(list.handle_event(down_event), EventResult::Ignored);

        // Bấm Right -> Active = true
        let right_event = KeyEvent::new(KeyCode::Right, crossterm::event::KeyModifiers::NONE);
        assert_eq!(list.handle_event(right_event), EventResult::Consumed);
        assert!(list.is_active());

        // Khi đã active, bấm Down sẽ di chuyển chọn item trong list (nhảy qua disabled)
        assert_eq!(list.handle_event(down_event), EventResult::Consumed);
        assert_eq!(list.selected(), 2);
        assert_eq!(list.selected_item().unwrap().text, "lib/utils.ts");

        // Bấm Left -> Thoát chế độ Active
        let left_event = KeyEvent::new(KeyCode::Left, crossterm::event::KeyModifiers::NONE);
        assert_eq!(list.handle_event(left_event), EventResult::Consumed);
        assert!(!list.is_active());
    }

    #[test]
    fn test_list_widget_form_value() {
        let list = ListWidget::new("files")
            .with_item("main.rs")
            .with_item("lib.rs");

        assert_eq!(
            list.value(),
            FormValue::Select(0, "main.rs".to_string())
        );
    }
}
