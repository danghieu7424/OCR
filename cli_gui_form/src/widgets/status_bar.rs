// --- PHÂN ĐOẠN: STATUS BAR WIDGET CHUẨN MINIMAL TUI DESIGN SYSTEM ---

use crate::theme::Theme;
use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

/****
 * Struct: StatusBarWidget
 * Chức năng: Hiển thị thanh trạng thái đáy 1 dòng theo chuẩn DESIGN.md mục 5.
 *            Left-aligned info (ngăn cách bởi ` ─ `), Right-aligned endpoint / status.
 * Ranh giới bảo vệ: Tự động tính toán khoảng cách căn 2 đầu (Left - Right justification).
 ****/
pub struct StatusBarWidget {
    left_segments: Vec<Span<'static>>,
    right_segments: Vec<Span<'static>>,
}

impl StatusBarWidget {
    pub fn new() -> Self {
        Self {
            left_segments: Vec::new(),
            right_segments: Vec::new(),
        }
    }

    pub fn with_left(mut self, segment: impl Into<Span<'static>>) -> Self {
        self.left_segments.push(segment.into());
        self
    }

    pub fn with_right(mut self, segment: impl Into<Span<'static>>) -> Self {
        self.right_segments.push(segment.into());
        self
    }

    pub fn add_left(&mut self, segment: impl Into<Span<'static>>) -> &mut Self {
        self.left_segments.push(segment.into());
        self
    }

    pub fn add_right(&mut self, segment: impl Into<Span<'static>>) -> &mut Self {
        self.right_segments.push(segment.into());
        self
    }

    pub fn render(&self, area: Rect, frame: &mut Frame) {
        let mut full_spans: Vec<Span<'static>> = Vec::new();

        // 1. Render Left Segments nối bằng " ─ "
        let mut left_len: usize = 0;
        for (i, span) in self.left_segments.iter().enumerate() {
            if i > 0 {
                full_spans.push(Span::styled(" ─ ", Style::default().fg(Theme::NEUTRAL_100)));
                left_len += 3;
            }
            full_spans.push(span.clone());
            left_len += span.content.chars().count();
        }

        // 2. Tính toán độ dài Right Segments
        let mut right_len: usize = 0;
        for (i, span) in self.right_segments.iter().enumerate() {
            if i > 0 {
                right_len += 3;
            }
            right_len += span.content.chars().count();
        }

        // 3. Chèn padding khoảng trắng ở giữa (Middle Spacer)
        let total_w = area.width as usize;
        let middle_space = if total_w > left_len + right_len {
            total_w - left_len - right_len
        } else {
            1
        };

        full_spans.push(Span::raw(" ".repeat(middle_space)));

        // 4. Render Right Segments nối bằng " ─ "
        for (i, span) in self.right_segments.iter().enumerate() {
            if i > 0 {
                full_spans.push(Span::styled(" ─ ", Style::default().fg(Theme::NEUTRAL_100)));
            }
            full_spans.push(span.clone());
        }

        let line = Line::from(full_spans);
        let paragraph = Paragraph::new(line).style(Style::default().bg(Theme::BG));
        frame.render_widget(paragraph, area);
    }
}

impl Default for StatusBarWidget {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_bar_creation() {
        let mut bar = StatusBarWidget::new();
        bar.add_left(Span::raw("main"));
        bar.add_right(Span::raw("127.0.0.1:3000"));
        assert_eq!(bar.left_segments.len(), 1);
        assert_eq!(bar.right_segments.len(), 1);
    }
}
