// --- PHÂN ĐOẠN: BUTTON VỚI ID FORM ---

use crate::{
    icons::Icons,
    traits::{EventResult, FormWidget},
};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame,
};

pub struct ButtonWidget {
    pub id: String,
    pub title: String,
    pub icon: Option<String>,
    pub bg_color: Color,
    pub fg_color: Color,
    pub focused_bg_color: Option<Color>,
    pub focused_fg_color: Option<Color>,
    pub bordered: bool,
    pub centered: bool,
    pub full_width: bool,
    pub bold_on_focus: bool,
    focused: bool,
}

impl ButtonWidget {
    pub fn new(id: impl Into<String>, title: impl Into<String>, bg_color: Color, fg_color: Color) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            icon: None,
            bg_color,
            fg_color,
            focused_bg_color: None,
            focused_fg_color: None,
            bordered: false,
            centered: false,
            full_width: false,
            bold_on_focus: false, // Mặc định tắt BOLD để bảo vệ font rendering và chống co rút glyph Unicode
            focused: false,
        }
    }

    /// Thêm biểu tượng icon (ví dụ: Icons::RUN, Icons::SUCCESS, Icons::STOP)
    pub fn with_icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// Tùy biến màu nền khi nút được chọn / hover
    pub fn with_focused_bg(mut self, bg: Color) -> Self {
        self.focused_bg_color = Some(bg);
        self
    }

    /// Tùy biến cả màu nền và màu chữ khi nút được chọn / hover
    pub fn with_focused_colors(mut self, bg: Color, fg: Color) -> Self {
        self.focused_bg_color = Some(bg);
        self.focused_fg_color = Some(fg);
        self
    }

    /// Áp dụng bộ màu chuẩn từ Theme (Theme::BTN_INVERT, Theme::BTN_ACCENT, Theme::BTN_PURPLE,...)
    /// Đảm bảo độ tương phản cao, an toàn tuyệt đối cho glyph Unicode không bao giờ bị co dẹt.
    pub fn with_preset(mut self, preset: (Color, Color, Color, Color)) -> Self {
        self.bg_color = preset.0;
        self.fg_color = preset.1;
        self.focused_bg_color = Some(preset.2);
        self.focused_fg_color = Some(preset.3);
        self
    }

    /// Đóng khung viền bo tròn (Rounded border) cho nút bấm dạng Box/Pill
    pub fn with_bordered(mut self, bordered: bool) -> Self {
        self.bordered = bordered;
        self
    }

    /// Căn giữa nội dung nút trong khung
    pub fn with_centered(mut self, centered: bool) -> Self {
        self.centered = centered;
        self
    }

    /// Tô màu nền toàn bộ chiều ngang container
    pub fn with_full_width(mut self, full_width: bool) -> Self {
        self.full_width = full_width;
        self
    }

    /// Tùy chọn có bật BOLD khi hover/focus hay không (mặc định false)
    pub fn with_bold(mut self, bold: bool) -> Self {
        self.bold_on_focus = bold;
        self
    }

    /****
     * Hàm: build_line
     * Chức năng: Xây dựng dòng hiển thị (Line) gồm các Span tách biệt hoàn toàn giữa Icon và Text.
     * Đầu vào: Tham chiếu &self của ButtonWidget.
     * Đầu ra: Line<'static> cấu trúc gồm Span Icon (Normal/Clean) và Span Text (sạch sẽ, chỉ bold nếu with_bold(true)).
     * Ranh giới bảo vệ:
     *   1. Span Icon tuyệt đối khóa cờ `remove_modifier(Modifier::BOLD)` để glyph Unicode 2-cell không bao giờ bị co méo.
     *   2. Span Text mặc định giữ nguyên Regular, chỉ nhận `Modifier::BOLD` khi cấu hình `with_bold(true)`.
     *   3. Không chèn thêm con trỏ `▸ ` khi nút đã có icon riêng nhằm chống biến dạng và lặp glyph.
     ****/
    pub fn build_line(&self) -> Line<'static> {
        let (prefix, bg, fg, is_focused) = if self.focused {
            let focused_bg = self.focused_bg_color.unwrap_or(crate::Theme::GRAY_33);
            let focused_fg = self.focused_fg_color.unwrap_or(crate::Theme::PRIMARY);
            ("▸ ", focused_bg, focused_fg, true)
        } else {
            ("  ", self.bg_color, self.fg_color, false)
        };

        // Bóc tách icon: Ưu tiên icon được thiết lập tường minh, sau đó quét tiền tố Icons::RUN trong title
        let mut display_title = self.title.clone();
        let icon_to_render = if let Some(ic) = &self.icon {
            Some(ic.clone())
        } else if let Some(stripped) = display_title.strip_prefix(Icons::RUN) {
            let res = Some(Icons::RUN.to_string());
            display_title = stripped.to_string();
            res
        } else if let Some(stripped) = display_title.strip_prefix(Icons::RUN.trim()) {
            let res = Some(Icons::RUN.to_string());
            display_title = stripped.trim_start().to_string();
            res
        } else {
            None
        };

        let has_icon = icon_to_render.is_some();
        let effective_prefix = if has_icon {
            // Khi đã có icon, nút đóng khung/căn giữa không cần tiền tố; nút inline giữ lề 1 space
            if self.bordered || self.centered { "" } else { " " }
        } else {
            prefix
        };

        let mut spans = Vec::new();
        if !effective_prefix.is_empty() {
            spans.push(Span::styled(
                format!(" {}", effective_prefix),
                Style::default().fg(fg).bg(bg),
            ));
        }

        if let Some(ic) = icon_to_render {
            let icon_str = if ic.ends_with(' ') {
                ic
            } else {
                format!("{} ", ic)
            };
            // Ranh giới bảo vệ thị giác: Icon tuyệt đối KHÔNG có Modifier::BOLD khi hover
            // để đảm bảo glyph 2-cell không bao giờ bị méo, co lại hay nhảy font trên terminal.
            let icon_style = Style::default()
                .fg(fg)
                .bg(bg)
                .remove_modifier(Modifier::BOLD);
            spans.push(Span::styled(icon_str, icon_style));
        }

        let mut title_style = Style::default().fg(fg).bg(bg);
        if is_focused && self.bold_on_focus {
            title_style = title_style.add_modifier(Modifier::BOLD);
        }
        spans.push(Span::styled(format!("{} ", display_title), title_style));

        Line::from(spans)
    }
}

impl FormWidget for ButtonWidget {
    fn id(&self) -> &str {
        &self.id
    }

    fn render(&self, area: Rect, frame: &mut Frame) {
        let (bg, fg) = if self.focused {
            let focused_bg = self.focused_bg_color.unwrap_or(crate::Theme::GRAY_33);
            let focused_fg = self.focused_fg_color.unwrap_or(crate::Theme::PRIMARY);
            (focused_bg, focused_fg)
        } else {
            (self.bg_color, self.fg_color)
        };

        let content = self.build_line();
        let mut paragraph = Paragraph::new(content);

        if self.centered {
            paragraph = paragraph.alignment(ratatui::layout::Alignment::Center);
        }

        if self.full_width {
            paragraph = paragraph.style(Style::default().bg(bg).fg(fg));
        }

        if self.bordered {
            let border_color = if self.focused {
                self.focused_fg_color.unwrap_or(crate::Theme::BORDER_FOCUS)
            } else {
                crate::Theme::NEUTRAL_100
            };
            let block = Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(border_color))
                .style(Style::default().bg(bg));
            paragraph = paragraph.block(block);
        }

        frame.render_widget(paragraph, area);
    }

    fn handle_event(&mut self, key: KeyEvent) -> EventResult {
        if key.kind != KeyEventKind::Press {
            return EventResult::Ignored;
        }

        match key.code {
            KeyCode::Enter | KeyCode::Char(' ') => EventResult::Submitted,
            _ => EventResult::Ignored,
        }
    }

    fn focus(&mut self) { self.focused = true; }
    fn blur(&mut self) { self.focused = false; }
    fn is_focused(&self) -> bool { self.focused }
    fn preferred_height(&self) -> u16 {
        if self.bordered { 3 } else { 2 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Theme;

    #[test]
    fn test_button_icon_never_bold_while_text_is_bold() {
        let mut btn = ButtonWidget::new(
            "btn_test",
            "▶ BẮT ĐẦU TẠO PODCAST (Enter)",
            Theme::GRAY_22,
            Theme::PRIMARY,
        )
        .with_bordered(true)
        .with_centered(true)
        .with_bold(true);
        btn.focus();

        let line = btn.build_line();
        let spans = line.spans;
        assert!(spans.len() >= 2);

        // Span 0 là icon "▶ " -> KHÔNG có BOLD, có cờ remove BOLD
        let icon_span = &spans[0];
        assert!(icon_span.content.contains('▶'));
        assert!(!icon_span.style.add_modifier.contains(Modifier::BOLD));
        assert!(icon_span.style.sub_modifier.contains(Modifier::BOLD));

        // Span 1 là text -> BẮT BUỘC có BOLD khi with_bold(true)
        let text_span = &spans[1];
        assert!(text_span.content.contains("BẮT ĐẦU TẠO PODCAST"));
        assert!(text_span.style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn test_button_default_no_bold_clean_rendering() {
        let mut btn = ButtonWidget::new("btn_clean", "▶ RUN", Theme::BG, Theme::PRIMARY);
        btn.focus();
        let line = btn.build_line();
        // Kiểm tra mặc định toàn bộ spans đều không bị dính cờ BOLD
        for span in &line.spans {
            assert!(!span.style.add_modifier.contains(Modifier::BOLD));
        }
    }

    #[test]
    fn test_button_with_explicit_icon_separation() {
        let mut btn = ButtonWidget::new("btn_run", "RUN JOB", Theme::BG, Theme::PRIMARY)
            .with_icon("▶")
            .with_bold(true);
        btn.focus();

        let line = btn.build_line();
        let spans = line.spans;
        // Do không có bordered/centered, spans[0] là prefix lề, spans[1] là icon, spans[2] là text
        let icon_span = spans.iter().find(|s| s.content.contains('▶')).expect("Icon span must exist");
        assert!(!icon_span.style.add_modifier.contains(Modifier::BOLD));
        assert!(icon_span.style.sub_modifier.contains(Modifier::BOLD));

        let text_span = spans.iter().find(|s| s.content.contains("RUN JOB")).expect("Text span must exist");
        assert!(text_span.style.add_modifier.contains(Modifier::BOLD));
    }
}