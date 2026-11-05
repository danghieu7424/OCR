// --- PHÂN ĐOẠN: CARD / PANEL WIDGET CHUẨN DESIGN.MD MỤC 5 ---

use crate::theme::Theme;
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

/****
 * Struct: CardItem
 * Chức năng: Lưu trữ thông tin một dòng dữ liệu/trạng thái trong Card (Deploy Status, Metrics, Resource...).
 * Ranh giới: Thuần dữ liệu, hỗ trợ icon và màu sắc trạng thái độc lập.
 ****/
#[derive(Clone, Debug)]
pub struct CardItem {
    pub label: String,
    pub status_icon: Option<String>,
    pub status_text: String,
    pub status_color: Color,
}

impl CardItem {
    pub fn new(
        label: impl Into<String>,
        status_icon: Option<impl Into<String>>,
        status_text: impl Into<String>,
        status_color: Color,
    ) -> Self {
        Self {
            label: label.into(),
            status_icon: status_icon.map(|s| s.into()),
            status_text: status_text.into(),
            status_color,
        }
    }
}

/****
 * Struct: CardWidget
 * Chức năng: Hiển thị Panel / Card đóng khung với tiêu đề gắn trên viền trên theo chuẩn DESIGN.md.
 *            Hỗ trợ cả viền góc vuông (Plain: ┌─ ... ─┐) và bo tròn (Rounded: ╭─ ... ─╮).
 * Ranh giới bảo vệ: Tự động tính toán padding 1-space inside và căn chỉnh khoảng cách giữa label và status badge.
 ****/
pub struct CardWidget {
    pub id: String,
    pub title: String,
    pub items: Vec<CardItem>,
    pub rounded: bool,
    pub border_color: Color,
    pub title_color: Color,
}

impl CardWidget {
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            items: Vec::new(),
            rounded: false,
            border_color: Theme::NEUTRAL_100,
            title_color: Theme::SECONDARY,
        }
    }

    pub fn with_rounded(mut self, rounded: bool) -> Self {
        self.rounded = rounded;
        self
    }

    pub fn with_border_color(mut self, color: Color) -> Self {
        self.border_color = color;
        self
    }

    pub fn with_title_color(mut self, color: Color) -> Self {
        self.title_color = color;
        self
    }

    pub fn with_item(
        mut self,
        label: impl Into<String>,
        status_icon: Option<impl Into<String>>,
        status_text: impl Into<String>,
        status_color: Color,
    ) -> Self {
        self.items.push(CardItem::new(label, status_icon, status_text, status_color));
        self
    }

    pub fn add_item(
        &mut self,
        label: impl Into<String>,
        status_icon: Option<impl Into<String>>,
        status_text: impl Into<String>,
        status_color: Color,
    ) -> &mut Self {
        self.items.push(CardItem::new(label, status_icon, status_text, status_color));
        self
    }

    pub fn preferred_height(&self) -> u16 {
        // 1 line top border + 1 line top padding + N item lines + 1 line bottom padding + 1 line bottom border
        (self.items.len() as u16) + 4
    }

    pub fn render(&self, area: Rect, frame: &mut Frame) {
        if area.width < 4 || area.height < 3 {
            return;
        }

        let width = area.width as usize;
        let inner_width = width.saturating_sub(2);

        // Ký tự viền theo cấu hình rounded / square
        let (tl, tr, bl, br, h, v) = if self.rounded {
            ("╭", "╮", "╰", "╯", "─", "│")
        } else {
            ("┌", "┐", "└", "┘", "─", "│")
        };

        let border_style = Style::default().fg(self.border_color);
        let mut lines: Vec<Line<'static>> = Vec::new();

        // 1. Top border có title lồng ở giữa: ┌─ Title ────────┐
        let title_str = format!(" {} ", self.title);
        let title_len = title_str.chars().count();
        let remaining_border = if inner_width > title_len + 1 {
            inner_width - title_len - 1
        } else {
            0
        };

        let mut top_spans = vec![
            Span::styled(format!("{}{}", tl, h), border_style),
            Span::styled(
                title_str,
                Style::default().fg(self.title_color).add_modifier(Modifier::BOLD),
            ),
        ];
        if remaining_border > 0 {
            top_spans.push(Span::styled(h.repeat(remaining_border), border_style));
        }
        top_spans.push(Span::styled(tr, border_style));
        lines.push(Line::from(top_spans));

        // 2. Top inner padding: │                              │
        lines.push(Line::from(vec![
            Span::styled(v, border_style),
            Span::raw(" ".repeat(inner_width)),
            Span::styled(v, border_style),
        ]));

        // 3. Nội dung các Card Items: │  Production    ✔ Ready       │
        // Tìm độ dài label lớn nhất để căn cột
        let max_label_len = self
            .items
            .iter()
            .map(|it| it.label.chars().count())
            .max()
            .unwrap_or(0);
        let label_col_width = (max_label_len + 4).max(14);

        for item in &self.items {
            let mut row_spans = vec![Span::styled(v, border_style)];
            
            // 2-space padding bên trái
            row_spans.push(Span::raw("  "));

            // Label
            let label_pad = if label_col_width > item.label.chars().count() {
                label_col_width - item.label.chars().count()
            } else {
                1
            };
            row_spans.push(Span::styled(
                item.label.clone(),
                Style::default().fg(Theme::FG),
            ));
            row_spans.push(Span::raw(" ".repeat(label_pad)));

            // Status Icon (nếu có)
            let mut status_content_len = 0;
            if let Some(ref icon) = item.status_icon {
                let icon_str = if icon.ends_with(' ') {
                    icon.clone()
                } else {
                    format!("{} ", icon)
                };
                let icon_len = icon_str.chars().count();
                row_spans.push(Span::styled(
                    icon_str,
                    Style::default().fg(item.status_color), // Không dùng BOLD trên glyph biểu tượng để chống clipping font trên Windows
                ));
                status_content_len += icon_len;
            }

            // Status Text
            row_spans.push(Span::styled(
                item.status_text.clone(),
                Style::default().fg(item.status_color).add_modifier(Modifier::BOLD),
            ));
            status_content_len += item.status_text.chars().count();

            // Khoảng trống còn lại trước viền phải
            let used_width = 2 + item.label.chars().count() + label_pad + status_content_len;
            let right_pad = if inner_width > used_width {
                inner_width - used_width
            } else {
                0
            };

            row_spans.push(Span::raw(" ".repeat(right_pad)));
            row_spans.push(Span::styled(v, border_style));
            lines.push(Line::from(row_spans));
        }

        // 4. Bottom inner padding: │                              │
        lines.push(Line::from(vec![
            Span::styled(v, border_style),
            Span::raw(" ".repeat(inner_width)),
            Span::styled(v, border_style),
        ]));

        // 5. Bottom border: └──────────────────────────────┘
        let bot_spans = vec![
            Span::styled(bl, border_style),
            Span::styled(h.repeat(inner_width), border_style),
            Span::styled(br, border_style),
        ];
        lines.push(Line::from(bot_spans));

        frame.render_widget(Paragraph::new(lines), area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_card_widget_creation() {
        let card = CardWidget::new("card_deploy", "Deploy Status")
            .with_item("Production", Some(crate::icons::Icons::CHECK.trim()), "Ready", Theme::SUCCESS)
            .with_item("Preview", Some(crate::icons::Icons::RUN.trim()), "Building", Theme::ACCENT)
            .with_item("Staging", Some(crate::icons::Icons::CHECK.trim()), "Ready", Theme::SUCCESS);

        assert_eq!(card.items.len(), 3);
        assert_eq!(card.title, "Deploy Status");
        assert_eq!(card.preferred_height(), 7);
    }
}
