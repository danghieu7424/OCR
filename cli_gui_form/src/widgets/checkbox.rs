// --- PHÂN ĐOẠN: CHECKBOX VỚI ID VÀ VALUE FORM ---

use crate::traits::{EventResult, FormValue, FormWidget};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

pub struct CheckboxWidget {
    pub id: String,
    pub label: String,
    pub checked: bool,
    focused: bool,
    pub focused_bg_color: Option<Color>,
    pub focused_fg_color: Option<Color>,
}

impl CheckboxWidget {
    pub fn new(id: impl Into<String>, label: impl Into<String>, checked: bool) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            checked,
            focused: false,
            // WHY: Mặc định màu focus theo chuẩn Vercel/Linear: nền Theme::ACCENT, chữ Theme::WHITE
            focused_bg_color: Some(crate::Theme::ACCENT),
            focused_fg_color: Some(crate::Theme::WHITE),
        }
    }

    /// Tùy biến màu khi focused theo phong cách Vercel/Design System
    pub fn with_focused_colors(mut self, bg: Color, fg: Color) -> Self {
        self.focused_bg_color = Some(bg);
        self.focused_fg_color = Some(fg);
        self
    }
}

impl FormWidget for CheckboxWidget {
    fn id(&self) -> &str {
        &self.id
    }

    fn value(&self) -> FormValue {
        FormValue::Bool(self.checked)
    }

    fn render(&self, area: Rect, frame: &mut Frame) {
        let (box_symbol, color) = if self.checked {
            (crate::Icons::CHECKBOX_ON, crate::Theme::SUCCESS)
        } else {
            (crate::Icons::CHECKBOX_OFF, crate::Theme::MUTED)
        };

        // Theo DESIGN.md mục 6: 2-space indent, 1-space padding sau icon
        let box_str = if box_symbol.ends_with(' ') {
            box_symbol.to_string()
        } else {
            format!("{} ", box_symbol)
        };

        let content = if self.focused {
            let bg = self.focused_bg_color.unwrap_or(crate::Theme::ACCENT);
            let fg = self.focused_fg_color.unwrap_or(crate::Theme::WHITE);
            let focus_style = Style::default().bg(bg).fg(fg).add_modifier(Modifier::BOLD);

            Line::from(vec![
                Span::styled("▸ ", Style::default().fg(crate::Theme::ACCENT).add_modifier(Modifier::BOLD)),
                Span::styled(
                    box_str,
                    if self.checked {
                        Style::default().bg(bg).fg(crate::Theme::SUCCESS).add_modifier(Modifier::BOLD)
                    } else {
                        focus_style
                    },
                ),
                Span::styled(format!("{} ", self.label), focus_style),
            ])
        } else {
            let label_style = Style::default().fg(crate::Theme::FG);
            Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled(box_str, Style::default().fg(color).add_modifier(Modifier::BOLD)),
                Span::styled(&self.label, label_style),
            ])
        };

        frame.render_widget(Paragraph::new(content), area);
    }

    fn handle_event(&mut self, key: KeyEvent) -> EventResult {
        if key.kind != KeyEventKind::Press {
            return EventResult::Ignored;
        }

        match key.code {
            KeyCode::Char(' ') | KeyCode::Enter => {
                self.checked = !self.checked;
                EventResult::Consumed
            }
            _ => EventResult::Ignored,
        }
    }

    fn focus(&mut self) { self.focused = true; }
    fn blur(&mut self) { self.focused = false; }
    fn is_focused(&self) -> bool { self.focused }
    fn preferred_height(&self) -> u16 { 2 }
}