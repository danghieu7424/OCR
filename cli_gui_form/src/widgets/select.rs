// --- PHÂN ĐOẠN: SELECT WIDGET DROPDOWN POPUP FLOATING OVERLAY CHUẨN HTML SELECT/OPTION ---

use crate::{
    icons::Icons,
    theme::Theme,
    traits::{EventResult, FormValue, FormWidget},
};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Padding, Paragraph},
    Frame,
};

/****
 * Struct: SelectOption
 * Chức năng: Đại diện cho một mục lựa chọn (<option>) bên trong SelectWidget.
 * Ranh giới bảo vệ: Hỗ trợ cặp giá trị value/label độc lập, kèm trạng thái disabled.
 ****/
#[derive(Clone, Debug, PartialEq)]
pub struct SelectOption {
    pub value: String,
    pub label: String,
    pub disabled: bool,
}

impl SelectOption {
    pub fn new(value: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            disabled: false,
        }
    }

    pub fn disabled(value: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            disabled: true,
        }
    }
}

impl From<&str> for SelectOption {
    fn from(s: &str) -> Self {
        Self::new(s, s)
    }
}

impl From<String> for SelectOption {
    fn from(s: String) -> Self {
        Self::new(s.clone(), s)
    }
}

impl<V: Into<String>, L: Into<String>> From<(V, L)> for SelectOption {
    fn from((v, l): (V, L)) -> Self {
        Self::new(v, l)
    }
}

/****
 * Struct: SelectWidget
 * Chức năng: Điều khiển chọn danh sách thả xuống dạng Dropdown Popup Overlay (<select> / <option>).
 *            Khi đóng: Chiếm 3 dòng cố định như ô input, hiển thị lựa chọn hiện tại hoặc placeholder.
 *            Khi mở: Bung menu popup nổi đè lên trên các widget bên dưới với viền bo tròn và Clear layer.
 * Ranh giới bảo vệ: Bắt trọn phím Up/Down/Enter/Esc khi popup mở; nhả phím Up/Down cho FormManager khi đóng.
 ****/
pub struct SelectWidget {
    pub id: String,
    pub label: String,
    pub options: Vec<SelectOption>,
    pub selected: Option<usize>,
    pub highlighted: usize,
    pub placeholder: String,
    pub is_open: bool,
    pub max_visible_options: usize,
    scroll_offset: usize,
    focused: bool,
}

impl SelectWidget {
    /****
     * Function: new
     * Chức năng: Khởi tạo SelectWidget với id, nhãn và danh sách các option ban đầu.
     ****/
    pub fn new(
        id: impl Into<String>,
        label: impl Into<String>,
        options: Vec<impl Into<SelectOption>>,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            options: options.into_iter().map(Into::into).collect(),
            selected: None,
            highlighted: 0,
            placeholder: String::new(),
            is_open: false,
            max_visible_options: 6,
            scroll_offset: 0,
            focused: false,
        }
    }

    pub fn with_placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    pub fn with_selected(mut self, index: usize) -> Self {
        if index < self.options.len() {
            self.selected = Some(index);
            self.highlighted = index;
        }
        self
    }

    pub fn with_option(mut self, value: impl Into<String>, label: impl Into<String>) -> Self {
        self.options.push(SelectOption::new(value, label));
        self
    }

    pub fn with_disabled_option(mut self, value: impl Into<String>, label: impl Into<String>) -> Self {
        self.options.push(SelectOption::disabled(value, label));
        self
    }

    pub fn with_max_visible(mut self, count: usize) -> Self {
        self.max_visible_options = count.max(1);
        self
    }

    pub fn selected_option(&self) -> Option<&SelectOption> {
        self.selected.and_then(|idx| self.options.get(idx))
    }

    pub fn selected_index(&self) -> Option<usize> {
        self.selected
    }

    pub fn is_open(&self) -> bool {
        self.is_open
    }

    pub fn open(&mut self) {
        self.is_open = true;
        self.highlighted = self.selected.unwrap_or(0);
        self.adjust_scroll();
    }

    pub fn close(&mut self) {
        self.is_open = false;
    }

    pub fn select_index(&mut self, index: usize) {
        if index < self.options.len() && !self.options[index].disabled {
            self.selected = Some(index);
            self.highlighted = index;
        }
    }

    fn adjust_scroll(&mut self) {
        if self.options.is_empty() {
            return;
        }
        let visible_count = self.options.len().min(self.max_visible_options);
        if self.highlighted < self.scroll_offset {
            self.scroll_offset = self.highlighted;
        } else if self.highlighted >= self.scroll_offset + visible_count {
            self.scroll_offset = self.highlighted + 1 - visible_count;
        }
    }
}

/****
 * Function: truncate_with_ellipsis
 * Chức năng: Cắt gọn văn bản nếu vượt quá chiều rộng hiển thị cho phép, thêm dấu '…' ở cuối.
 * Ranh giới bảo vệ: Tránh panic trên UTF-8 bằng iterator chars, đảm bảo không đẩy rớt border hoặc scrollbar.
 ****/
fn truncate_with_ellipsis(text: &str, max_len: usize) -> String {
    let char_count = text.chars().count();
    if char_count <= max_len {
        text.to_string()
    } else if max_len <= 1 {
        text.chars().take(max_len).collect()
    } else {
        let mut truncated: String = text.chars().take(max_len - 1).collect();
        truncated.push('…');
        truncated
    }
}

impl FormWidget for SelectWidget {
    fn id(&self) -> &str {
        &self.id
    }

    fn value(&self) -> FormValue {
        if let Some(idx) = self.selected {
            if let Some(opt) = self.options.get(idx) {
                return FormValue::Select(idx, opt.value.clone());
            }
        }
        FormValue::None
    }

    fn render(&self, area: Rect, frame: &mut Frame) {
        if area.width < 4 || area.height < 3 {
            return;
        }

        // 1. Xác định màu viền và mũi tên trạng thái đóng/mở
        let border_color = if self.focused {
            Theme::BORDER_FOCUS
        } else {
            Theme::MUTED
        };

        let arrow = if self.is_open { "▴" } else { "▾" };
        let arrow_color = if self.focused {
            Theme::ACCENT
        } else {
            Theme::SECONDARY
        };

        // 2. Nội dung hiển thị bên trong ô
        let (display_text, text_style) = if let Some(idx) = self.selected {
            if let Some(opt) = self.options.get(idx) {
                let style = if self.focused {
                    Style::default().fg(Theme::PRIMARY).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Theme::FG)
                };
                (opt.label.clone(), style)
            } else {
                let ph = if self.placeholder.is_empty() { "Select an option..." } else { &self.placeholder };
                (ph.to_string(), Style::default().fg(Theme::NEUTRAL_300))
            }
        } else {
            let ph = if self.placeholder.is_empty() { "Select an option..." } else { &self.placeholder };
            (ph.to_string(), Style::default().fg(Theme::NEUTRAL_300))
        };

        let title_formatted = format!("─ {} ─", self.label);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(border_color))
            .padding(Padding::horizontal(1))
            .title(title_formatted);

        // Tính khoảng trống để đẩy mũi tên ▾ về sát mép phải
        let inner_width = (area.width as usize).saturating_sub(4); // 2 viền + 2 padding
        // Chừa ít nhất 2 ký tự: 1 ký tự khoảng cách phân tách và 1 ký tự mũi tên '▾'
        let max_text_len = inner_width.saturating_sub(2);
        let display_text_safe = truncate_with_ellipsis(&display_text, max_text_len);
        let text_chars = display_text_safe.chars().count();
        let spacing = inner_width.saturating_sub(text_chars + 1); // 1 char cho arrow

        let line = Line::from(vec![
            Span::styled(display_text_safe, text_style),
            Span::raw(" ".repeat(spacing)),
            Span::styled(arrow, Style::default().fg(arrow_color)),
        ]);

        let paragraph = Paragraph::new(line).block(block);
        frame.render_widget(paragraph, area);
    }

    /****
     * Function: render_overlay
     * Chức năng: Vẽ menu popup nổi (Floating Dropdown) đè lên trên các widget bên dưới khi is_open = true.
     * Ranh giới bảo vệ: Dùng Clear widget quét sạch cell bên dưới, viền bo tròn, tự lật lên nếu kẹt đáy màn hình.
     ****/
    fn render_overlay(&self, area: Rect, frame: &mut Frame) {
        if !self.is_open || self.options.is_empty() {
            return;
        }

        let total_opts = self.options.len();
        let visible_count = total_opts.min(self.max_visible_options);
        let popup_h = (visible_count as u16) + 2; // + 2 viền trên/dưới
        let screen_h = frame.area().height;
        let screen_w = frame.area().width;

        // Tự động kiểm tra không gian: nếu phía dưới không đủ chỗ thì lật ngược lên trên ô select
        let space_below = screen_h.saturating_sub(area.y + area.height);
        let popup_y = if space_below >= popup_h {
            area.y + area.height
        } else {
            area.y.saturating_sub(popup_h)
        };

        let popup_w = area.width.min(screen_w);
        let popup_x = area.x.min(screen_w.saturating_sub(popup_w));

        let popup_area = Rect {
            x: popup_x,
            y: popup_y,
            width: popup_w,
            height: popup_h,
        };

        // 1. Quét sạch vùng bên dưới để chống chữ xuyên thấu
        frame.render_widget(Clear, popup_area);

        // 2. Tính toán thanh cuộn (Scrollbar thumb) khi tổng số option vượt quá max_visible_options
        let has_scroll = total_opts > visible_count;
        let thumb_len = if has_scroll {
            ((visible_count * visible_count) / total_opts).max(1)
        } else {
            0
        };
        let max_scroll = total_opts.saturating_sub(visible_count);
        let max_start = visible_count.saturating_sub(thumb_len);
        let thumb_start = if has_scroll && max_scroll > 0 {
            (self.scroll_offset * max_start + (max_scroll / 2)) / max_scroll
        } else {
            0
        };

        // 3. Khung viền nổi bật bo góc chuẩn Linear/Vercel kèm chỉ báo số lượng [idx/total]
        let mut block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Theme::BORDER_FOCUS))
            .style(Style::default().bg(Theme::SURFACE_ELEVATED))
            .padding(Padding::horizontal(1));

        if has_scroll {
            block = block.title(format!("─ [{}/{}] ─", self.highlighted + 1, total_opts));
        }

        // 4. Render danh sách các option với thanh cuộn tinh tế ở mép phải
        let mut lines = Vec::new();
        let end_idx = (self.scroll_offset + visible_count).min(total_opts);
        let inner_w = (popup_w as usize).saturating_sub(4); // 2 viền + 2 padding
        let scroll_col_width = if has_scroll { 2 } else { 0 }; // 1 char track/thumb + 1 space cách lề

        for (row_idx, idx) in (self.scroll_offset..end_idx).enumerate() {
            let opt = &self.options[idx];
            let is_highlighted = idx == self.highlighted;
            let is_selected = Some(idx) == self.selected;

            let mut spans = Vec::new();

            // Tính toán trước độ rộng các thành phần cố định để bảo vệ scrollbar mép phải
            let prefix_w = 2; // Icons::POINTER_FILLED hoặc "  "
            let check_w = if is_selected { Icons::CHECK.chars().count() + 1 } else { 0 };
            let disabled_w = if opt.disabled { 11 } else { 0 }; // " (disabled)"
            let fixed_w = prefix_w + check_w + disabled_w + scroll_col_width;
            let max_label_w = inner_w.saturating_sub(fixed_w);
            let safe_label = truncate_with_ellipsis(&opt.label, max_label_w);

            if is_highlighted {
                // Mục đang được rê qua: con trỏ Icons::POINTER_FILLED và Primary BOLD
                spans.push(Span::styled(Icons::POINTER_FILLED, Style::default().fg(Theme::PRIMARY).add_modifier(Modifier::BOLD)));
                let text_style = if opt.disabled {
                    Style::default().fg(Theme::NEUTRAL_200).add_modifier(Modifier::DIM)
                } else {
                    Style::default().fg(Theme::PRIMARY).add_modifier(Modifier::BOLD)
                };
                spans.push(Span::styled(safe_label, text_style));
            } else {
                spans.push(Span::raw("  "));
                let text_style = if opt.disabled {
                    Style::default().fg(Theme::NEUTRAL_200).add_modifier(Modifier::DIM)
                } else {
                    Style::default().fg(Theme::FG)
                };
                spans.push(Span::styled(safe_label, text_style));
            }

            // Biểu tượng checkmark nếu đã được chọn (chuẩn Icons::CHECK 1 cell)
            if is_selected {
                spans.push(Span::styled(format!(" {}", Icons::CHECK.trim()), Style::default().fg(Theme::SUCCESS)));
            }

            if opt.disabled {
                spans.push(Span::styled(" (disabled)", Style::default().fg(Theme::NEUTRAL_300).add_modifier(Modifier::DIM)));
            }

            // Thanh cuộn ở mép phải của mỗi hàng
            if has_scroll {
                let cur_len: usize = spans.iter().map(|s| s.content.chars().count()).sum();
                let pad = inner_w.saturating_sub(cur_len + 1);
                if pad > 0 {
                    spans.push(Span::raw(" ".repeat(pad)));
                }

                if row_idx >= thumb_start && row_idx < thumb_start + thumb_len {
                    spans.push(Span::styled("█", Style::default().fg(Theme::PRIMARY)));
                } else {
                    spans.push(Span::styled("│", Style::default().fg(Theme::NEUTRAL_100)));
                }
            }

            lines.push(Line::from(spans));
        }

        let paragraph = Paragraph::new(lines).block(block);
        frame.render_widget(paragraph, popup_area);
    }

    fn handle_event(&mut self, key: KeyEvent) -> EventResult {
        if key.kind != KeyEventKind::Press {
            return EventResult::Ignored;
        }

        if !self.is_open {
            // Khi đóng: Chỉ nhận Enter hoặc Space để mở menu popup; Up/Down nhả cho FormManager chuyển ô
            match key.code {
                KeyCode::Enter | KeyCode::Char(' ') => {
                    self.open();
                    EventResult::Consumed
                }
                _ => EventResult::Ignored,
            }
        } else {
            // Khi mở: Bắt trọn vẹn sự kiện điều hướng nội bộ menu dropdown
            match key.code {
                KeyCode::Esc => {
                    self.close();
                    EventResult::Consumed
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    if self.highlighted > 0 {
                        self.highlighted -= 1;
                        if self.options[self.highlighted].disabled && self.highlighted > 0 {
                            self.highlighted -= 1;
                        }
                    } else if !self.options.is_empty() {
                        self.highlighted = self.options.len() - 1;
                    }
                    self.adjust_scroll();
                    EventResult::Consumed
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if self.highlighted + 1 < self.options.len() {
                        self.highlighted += 1;
                        if self.options[self.highlighted].disabled && self.highlighted + 1 < self.options.len() {
                            self.highlighted += 1;
                        }
                    } else {
                        self.highlighted = 0;
                    }
                    self.adjust_scroll();
                    EventResult::Consumed
                }
                KeyCode::Home => {
                    self.highlighted = 0;
                    self.adjust_scroll();
                    EventResult::Consumed
                }
                KeyCode::End => {
                    if !self.options.is_empty() {
                        self.highlighted = self.options.len() - 1;
                        self.adjust_scroll();
                    }
                    EventResult::Consumed
                }
                KeyCode::PageUp => {
                    let step = self.max_visible_options.min(self.highlighted);
                    self.highlighted = self.highlighted.saturating_sub(step);
                    self.adjust_scroll();
                    EventResult::Consumed
                }
                KeyCode::PageDown => {
                    let step = self.max_visible_options;
                    self.highlighted = (self.highlighted + step).min(self.options.len().saturating_sub(1));
                    self.adjust_scroll();
                    EventResult::Consumed
                }
                KeyCode::Enter | KeyCode::Char(' ') => {
                    if let Some(opt) = self.options.get(self.highlighted) {
                        if !opt.disabled {
                            self.selected = Some(self.highlighted);
                            self.close();
                        }
                    }
                    EventResult::Consumed
                }
                KeyCode::Tab => {
                    // Nhấn Tab sẽ đóng popup và cho phép chuyển ô tiếp theo
                    self.close();
                    EventResult::Ignored
                }
                _ => EventResult::Consumed, // Nuốt các phím gõ linh tinh để không làm ảnh hưởng form
            }
        }
    }

    fn focus(&mut self) {
        self.focused = true;
    }

    fn blur(&mut self) {
        self.focused = false;
        self.is_open = false; // Tự động đóng dropdown nếu mất focus
    }

    fn is_focused(&self) -> bool {
        self.focused
    }

    fn preferred_height(&self) -> u16 {
        3
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_select_widget_basic_creation_and_value() {
        let mut select = SelectWidget::new(
            "env",
            "Environment",
            vec!["dev", "staging", "prod"],
        )
        .with_placeholder("Choose an environment...");

        assert_eq!(select.value(), FormValue::None);
        assert_eq!(select.placeholder, "Choose an environment...");
        assert!(!select.is_open());

        // Chọn index 1 ("staging")
        select.select_index(1);
        assert_eq!(
            select.value(),
            FormValue::Select(1, "staging".to_string())
        );
        assert_eq!(select.selected_option().unwrap().value, "staging");
    }

    #[test]
    fn test_select_widget_open_navigate_and_select() {
        let mut select = SelectWidget::new(
            "region",
            "AWS Region",
            vec![
                ("us-east", "US East (N. Virginia)"),
                ("eu-central", "Europe (Frankfurt)"),
                ("ap-southeast", "Asia Pacific (Singapore)"),
            ],
        );

        // Ban đầu đóng, Up/Down bị ignore
        assert_eq!(
            select.handle_event(KeyEvent::from(KeyCode::Down)),
            EventResult::Ignored
        );

        // Bấm Enter -> Mở popup
        assert_eq!(
            select.handle_event(KeyEvent::from(KeyCode::Enter)),
            EventResult::Consumed
        );
        assert!(select.is_open());
        assert_eq!(select.highlighted, 0);

        // Khi mở -> Down di chuyển highlight
        let _ = select.handle_event(KeyEvent::from(KeyCode::Down));
        assert_eq!(select.highlighted, 1);

        let _ = select.handle_event(KeyEvent::from(KeyCode::Down));
        assert_eq!(select.highlighted, 2);

        // Bấm Enter -> Chọn option 2 ("ap-southeast") và đóng popup
        let _ = select.handle_event(KeyEvent::from(KeyCode::Enter));
        assert!(!select.is_open());
        assert_eq!(
            select.value(),
            FormValue::Select(2, "ap-southeast".to_string())
        );
    }

    #[test]
    fn test_select_widget_esc_cancels() {
        let mut select = SelectWidget::new("tz", "Timezone", vec!["UTC", "GMT", "PST"])
            .with_selected(0);

        select.open();
        assert!(select.is_open());

        // Di chuyển highlight tới GMT (index 1)
        let _ = select.handle_event(KeyEvent::from(KeyCode::Down));
        assert_eq!(select.highlighted, 1);

        // Nhấn Esc huỷ bỏ
        let _ = select.handle_event(KeyEvent::from(KeyCode::Esc));
        assert!(!select.is_open());
        // Giữ nguyên lựa chọn ban đầu (index 0)
        assert_eq!(select.selected, Some(0));
    }

    #[test]
    fn test_select_widget_dozens_of_options_scrolling_and_navigation() {
        // Tạo 50 options (ví dụ 50 bang/tiểu bang hoặc 50 quốc gia)
        let options: Vec<String> = (1..=50).map(|i| format!("Option {:02}", i)).collect();
        let mut select = SelectWidget::new("fifty", "Dozens of Options", options)
            .with_max_visible(6);

        select.open();
        assert!(select.is_open());
        assert_eq!(select.highlighted, 0);
        assert_eq!(select.scroll_offset, 0);

        // 1. Phím End: Nhảy ngay tới option cuối cùng (49)
        let _ = select.handle_event(KeyEvent::from(KeyCode::End));
        assert_eq!(select.highlighted, 49);
        // scroll_offset phải tự động cuộn đến 49 + 1 - 6 = 44
        assert_eq!(select.scroll_offset, 44);

        // 2. Phím Home: Nhảy về option đầu tiên (0)
        let _ = select.handle_event(KeyEvent::from(KeyCode::Home));
        assert_eq!(select.highlighted, 0);
        assert_eq!(select.scroll_offset, 0);

        // 3. Phím PageDown: Nhảy xuống 6 options
        let _ = select.handle_event(KeyEvent::from(KeyCode::PageDown));
        assert_eq!(select.highlighted, 6);
        assert_eq!(select.scroll_offset, 1);

        // 4. Phím PageUp: Nhảy ngược lên
        let _ = select.handle_event(KeyEvent::from(KeyCode::PageUp));
        assert_eq!(select.highlighted, 0);
        assert_eq!(select.scroll_offset, 0);

        // 5. Chọn option 12 bằng phím
        select.highlighted = 12;
        let _ = select.handle_event(KeyEvent::from(KeyCode::Enter));
        assert!(!select.is_open());
        assert_eq!(select.value(), FormValue::Select(12, "Option 13".to_string()));
    }

    #[test]
    fn test_truncate_with_ellipsis_and_overflow_protection() {
        assert_eq!(truncate_with_ellipsis("Short", 10), "Short");
        assert_eq!(truncate_with_ellipsis("Exactly10chars", 14), "Exactly10chars");
        assert_eq!(truncate_with_ellipsis("Very Long Text That Exceeds Width", 10), "Very Long…");
        assert_eq!(truncate_with_ellipsis("A", 1), "A");
        assert_eq!(truncate_with_ellipsis("ABC", 0), "");
    }
}

