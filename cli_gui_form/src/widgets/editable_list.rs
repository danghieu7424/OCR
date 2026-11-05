// --- PHÂN ĐOẠN: ATOMIC WIDGET - EDITABLE LIST (DYNAMIC CRUD OPTIONS) ---
// Kiến trúc: Hybrid FSD + Atomic Component
// Tiêu chuẩn thẩm mỹ: Linear / Vercel TUI (Border Rounded, Zero-Noise, Safe Ellipsis Truncation)

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Padding, Paragraph},
    Frame,
};

use crate::{
    theme::Theme,
    traits::{EventResult, FormValue, FormWidget},
};

/****
 * Enum: EditMode
 * Chức năng: Quản lý trạng thái con trỏ và chế độ nhập liệu nội tại của EditableListWidget.
 * Ranh giới bảo vệ: Phân tách rõ ràng giữa duyệt danh sách (Browsing) và sửa/thêm inline (Adding/Editing).
 ****/
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditMode {
    Browsing,
    Adding {
        input: String,
        cursor: usize,
    },
    Editing {
        index: usize,
        input: String,
        cursor: usize,
    },
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

/****
 * Function: render_input_spans
 * Chức năng: Tạo các Span hiển thị văn bản kèm con trỏ insert dạng khối '█' di chuyển theo cursor.
 * Ranh giới bảo vệ: Đảm bảo con trỏ '█' luôn nằm trong khung hiển thị và di chuyển trực quan khi nhấn Left/Right.
 ****/
fn render_input_spans(input: &str, cursor: usize, max_w: usize) -> Vec<Span<'static>> {
    let chars: Vec<char> = input.chars().collect();
    let total_chars = chars.len();
    let cursor = cursor.min(total_chars);

    // Dành 1 ô cho ký tự con trỏ █
    let view_capacity = max_w.saturating_sub(1).max(1);
    let start_idx = if total_chars <= view_capacity {
        0
    } else if cursor >= view_capacity {
        cursor + 1 - view_capacity
    } else {
        0
    };
    let end_idx = (start_idx + view_capacity).min(total_chars);

    let mut spans = Vec::new();
    let left: String = chars[start_idx..cursor].iter().collect();
    if !left.is_empty() {
        spans.push(Span::styled(left, Style::default().fg(Theme::FG).add_modifier(Modifier::BOLD)));
    }

    // Con trỏ chèn dạng khối █ chuẩn Linear TUI
    spans.push(Span::styled("█", Style::default().fg(Theme::PRIMARY)));

    let right: String = chars[cursor..end_idx].iter().collect();
    if !right.is_empty() {
        spans.push(Span::styled(right, Style::default().fg(Theme::FG).add_modifier(Modifier::BOLD)));
    }

    spans
}

/****
 * Struct: EditableListWidget
 * Chức năng: Widget danh sách động kết hợp giữa Select/Option và Text Input (Dynamic CRUD List).
 * Ranh giới bảo vệ:
 * - Khi đóng: Thu gọn dạng `[+] Title` (rỗng) hoặc `[n] Title` (có n mục).
 * - Khi mở: Popup nổi (Floating Overlay), tự động lật lên trên nếu kẹt đáy màn hình.
 * - Thao tác: Hỗ trợ Thêm mới (+), Chỉnh sửa (Enter/→), Xoá (Delete/Backspace), Huỷ (Esc).
 * - Bảo vệ tràn chữ: Cắt tỉa an toàn bằng `…`, giữ cố định vị trí thanh cuộn mép phải.
 ****/
pub struct EditableListWidget {
    pub id: String,
    pub label: String,
    pub items: Vec<String>,
    pub is_open: bool,
    pub focused: bool,
    pub highlighted: usize, // 0: Dòng "+ Thêm mới.", 1..=items.len(): Item thứ (highlighted - 1)
    pub scroll_offset: usize,
    pub max_visible: usize,
    pub max_items: Option<usize>,
    pub mode: EditMode,
}

impl EditableListWidget {
    /****
     * Function: new
     * Chức năng: Khởi tạo EditableListWidget mới với danh sách items ban đầu (nếu có).
     * Ranh giới bảo vệ: highlighted mặc định trỏ vào dòng 0 (+ Thêm mới) nếu rỗng, hoặc dòng 1 nếu có sẵn items.
     ****/
    pub fn new<I, S>(id: &str, label: &str, initial_items: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let items: Vec<String> = initial_items.into_iter().map(|s| s.into()).collect();
        let highlighted = if items.is_empty() { 0 } else { 1 };
        Self {
            id: id.to_string(),
            label: label.to_string(),
            items,
            is_open: false,
            focused: false,
            highlighted,
            scroll_offset: 0,
            max_visible: 6,
            max_items: None,
            mode: EditMode::Browsing,
        }
    }

    pub fn with_max_visible(mut self, max: usize) -> Self {
        self.max_visible = max.max(2);
        self
    }

    pub fn with_max_items(mut self, max: usize) -> Self {
        self.max_items = Some(max);
        self
    }

    pub fn is_open(&self) -> bool {
        self.is_open
    }

    pub fn open(&mut self) {
        self.is_open = true;
        self.mode = EditMode::Browsing;
        self.adjust_scroll();
    }

    pub fn close(&mut self) {
        self.is_open = false;
        self.mode = EditMode::Browsing;
    }

    /****
     * Function: adjust_scroll
     * Chức năng: Tự động cuộn thanh viewport theo con trỏ highlighted (Sticky follow).
     * Ranh giới bảo vệ: Ngăn chặn tràn chỉ mục khi danh sách thay đổi kích thước sau khi xoá/thêm.
     ****/
    fn adjust_scroll(&mut self) {
        let total_rows = 1 + self.items.len(); // Dòng 0 luôn là "+ Thêm mới."
        if total_rows == 0 {
            self.highlighted = 0;
            self.scroll_offset = 0;
            return;
        }

        if self.highlighted >= total_rows {
            self.highlighted = total_rows.saturating_sub(1);
        }

        let visible_count = total_rows.min(self.max_visible);
        if self.highlighted < self.scroll_offset {
            self.scroll_offset = self.highlighted;
        } else if self.highlighted >= self.scroll_offset + visible_count {
            self.scroll_offset = self.highlighted + 1 - visible_count;
        }
    }

    /****
     * Function: handle_text_key
     * Chức năng: Điều phối phím văn bản UTF-8 an toàn cho buffer đang thêm mới hoặc sửa đổi.
     * Ranh giới bảo vệ: Bảo vệ con trỏ ký tự không rơi vào giữa byte UTF-8.
     ****/
    fn handle_text_key(input: &mut String, cursor: &mut usize, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Char(c) => {
                let char_idx = *cursor;
                let byte_idx = input.chars().take(char_idx).map(|ch| ch.len_utf8()).sum();
                input.insert(byte_idx, c);
                *cursor += 1;
                true
            }
            KeyCode::Backspace => {
                if *cursor > 0 {
                    let char_idx = *cursor - 1;
                    let byte_idx = input.chars().take(char_idx).map(|ch| ch.len_utf8()).sum();
                    input.remove(byte_idx);
                    *cursor -= 1;
                    true
                } else {
                    false
                }
            }
            KeyCode::Delete => {
                let total_chars = input.chars().count();
                if *cursor < total_chars {
                    let char_idx = *cursor;
                    let byte_idx = input.chars().take(char_idx).map(|ch| ch.len_utf8()).sum();
                    input.remove(byte_idx);
                    true
                } else {
                    false
                }
            }
            KeyCode::Left => {
                if *cursor > 0 {
                    *cursor -= 1;
                    true
                } else {
                    false
                }
            }
            KeyCode::Right => {
                let total_chars = input.chars().count();
                if *cursor < total_chars {
                    *cursor += 1;
                    true
                } else {
                    false
                }
            }
            KeyCode::Home => {
                *cursor = 0;
                true
            }
            KeyCode::End => {
                *cursor = input.chars().count();
                true
            }
            _ => false,
        }
    }
}

impl FormWidget for EditableListWidget {
    fn id(&self) -> &str {
        &self.id
    }

    fn value(&self) -> FormValue {
        FormValue::List(self.items.clone())
    }

    fn preferred_height(&self) -> u16 {
        3
    }

    fn focus(&mut self) {
        self.focused = true;
    }

    fn blur(&mut self) {
        self.focused = false;
        self.close();
    }

    fn is_focused(&self) -> bool {
        self.focused
    }

    /****
     * Function: render
     * Chức năng: Hiển thị trạng thái đóng (Collapsed) của widget trong Form layout.
     * Ranh giới bảo vệ: Khung bo góc, hiển thị `[+] Title` khi rỗng hoặc `[n] Title` khi có n mục.
     ****/
    fn render(&self, area: Rect, frame: &mut Frame) {
        if area.width < 4 || area.height < 3 {
            return;
        }

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

        // Quy tắc hiển thị Collapsed:
        // [+] Title. khi chưa có item
        // [n] Title. khi có n items
        let count_str = if self.items.is_empty() {
            "[+]".to_string()
        } else {
            format!("[{}]", self.items.len())
        };

        let display_text = format!("{} {}", count_str, self.label);
        let text_style = if self.focused {
            Style::default().fg(Theme::PRIMARY).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Theme::FG)
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(border_color))
            .padding(Padding::horizontal(1));

        let inner_width = (area.width as usize).saturating_sub(4);
        let max_text_len = inner_width.saturating_sub(2);
        let display_text_safe = truncate_with_ellipsis(&display_text, max_text_len);
        let text_chars = display_text_safe.chars().count();
        let spacing = inner_width.saturating_sub(text_chars + 1);

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
     * Chức năng: Vẽ Floating Dropdown danh sách các option, hỗ trợ CRUD inline và Scrollbar mép phải.
     * Ranh giới bảo vệ: Quét sạch cell bên dưới bằng Clear, tự động lật lên trên nếu kẹt đáy màn hình.
     ****/
    fn render_overlay(&self, area: Rect, frame: &mut Frame) {
        if !self.is_open {
            return;
        }

        let total_rows = 1 + self.items.len();
        let visible_count = total_rows.min(self.max_visible);
        let popup_h = (visible_count as u16) + 2;
        let screen_h = frame.area().height;
        let screen_w = frame.area().width;

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

        // 1. Quét sạch bề mặt bên dưới chống xuyên thấu text
        frame.render_widget(Clear, popup_area);

        // 2. Tính toán Scrollbar track & thumb cố định
        let has_scroll = total_rows > visible_count;
        let thumb_len = if has_scroll {
            ((visible_count * visible_count) / total_rows).max(1)
        } else {
            0
        };
        let max_scroll = total_rows.saturating_sub(visible_count);
        let max_start = visible_count.saturating_sub(thumb_len);
        let thumb_start = if has_scroll && max_scroll > 0 {
            (self.scroll_offset * max_start + (max_scroll / 2)) / max_scroll
        } else {
            0
        };

        // 3. Khung viền Floating bo tròn
        let mut block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Theme::BORDER_FOCUS))
            .style(Style::default().bg(Theme::SURFACE_ELEVATED))
            .padding(Padding::horizontal(1));

        if has_scroll {
            block = block.title(format!("─ [{}/{}] ─", self.highlighted + 1, total_rows));
        } else {
            block = block.title(format!("─ [{}] ─", self.items.len()));
        }

        // 4. Render các dòng options & inline edit input
        let mut lines = Vec::new();
        let end_idx = (self.scroll_offset + visible_count).min(total_rows);
        let inner_w = (popup_w as usize).saturating_sub(4);
        let scroll_col_width = if has_scroll { 2 } else { 0 };

        for (row_idx, idx) in (self.scroll_offset..end_idx).enumerate() {
            let is_highlighted = idx == self.highlighted;
            let mut spans = Vec::new();

            if idx == 0 {
                // DÒNG 0: "[+] Thêm mới."
                match &self.mode {
                    EditMode::Adding { input, cursor } if is_highlighted => {
                        spans.push(Span::styled("▸ [+] [ ", Style::default().fg(Theme::PRIMARY).add_modifier(Modifier::BOLD)));
                        let max_input_w = inner_w.saturating_sub(10 + scroll_col_width);
                        spans.extend(render_input_spans(input, *cursor, max_input_w));
                        spans.push(Span::styled(" ]", Style::default().fg(Theme::PRIMARY)));
                    }
                    _ => {
                        let can_add = self.max_items.map_or(true, |max| self.items.len() < max);
                        if is_highlighted {
                            spans.push(Span::styled("▸ ", Style::default().fg(Theme::PRIMARY).add_modifier(Modifier::BOLD)));
                            let text = if can_add {
                                "[+] Thêm mới."
                            } else {
                                "[+] Thêm mới. (Đầy)"
                            };
                            spans.push(Span::styled(text, Style::default().fg(Theme::PRIMARY).add_modifier(Modifier::BOLD)));
                        } else {
                            spans.push(Span::raw("  "));
                            let text = if can_add {
                                "[+] Thêm mới."
                            } else {
                                "[+] Thêm mới. (Đầy)"
                            };
                            let style = if can_add {
                                Style::default().fg(Theme::SECONDARY)
                            } else {
                                Style::default().fg(Theme::NEUTRAL_200).add_modifier(Modifier::DIM)
                            };
                            spans.push(Span::styled(text, style));
                        }
                    }
                }
            } else {
                // DÒNG 1..=n: Items dữ liệu
                let item_idx = idx - 1;
                let item_text = &self.items[item_idx];
                let prefix_num = format!("[{}] ", item_idx + 1);

                match &self.mode {
                    EditMode::Editing { index, input, cursor } if *index == item_idx && is_highlighted => {
                        spans.push(Span::styled("▸ ", Style::default().fg(Theme::PRIMARY).add_modifier(Modifier::BOLD)));
                        spans.push(Span::styled(prefix_num.clone(), Style::default().fg(Theme::SECONDARY)));
                        spans.push(Span::styled("[ ", Style::default().fg(Theme::PRIMARY)));
                        let fixed_w = 2 + prefix_num.chars().count() + 2 + 2 + scroll_col_width;
                        let max_input_w = inner_w.saturating_sub(fixed_w);
                        spans.extend(render_input_spans(input, *cursor, max_input_w));
                        spans.push(Span::styled(" ]", Style::default().fg(Theme::PRIMARY)));
                    }
                    _ => {
                        let prefix_w = 2 + prefix_num.chars().count();
                        let max_text_w = inner_w.saturating_sub(prefix_w + scroll_col_width);
                        let safe_text = truncate_with_ellipsis(item_text, max_text_w);

                        if is_highlighted {
                            spans.push(Span::styled("▸ ", Style::default().fg(Theme::PRIMARY).add_modifier(Modifier::BOLD)));
                            spans.push(Span::styled(prefix_num, Style::default().fg(Theme::PRIMARY).add_modifier(Modifier::BOLD)));
                            spans.push(Span::styled(safe_text, Style::default().fg(Theme::PRIMARY).add_modifier(Modifier::BOLD)));
                        } else {
                            spans.push(Span::raw("  "));
                            spans.push(Span::styled(prefix_num, Style::default().fg(Theme::SECONDARY)));
                            spans.push(Span::styled(safe_text, Style::default().fg(Theme::FG)));
                        }
                    }
                }
            }

            // Thanh cuộn mép phải ổn định
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

    /****
     * Function: handle_event
     * Chức năng: Điều khiển máy trạng thái phím (State Machine Key Handler).
     * Ranh giới bảo vệ:
     * - Khi đang gõ: Enter để Lưu, Esc để Huỷ.
     * - Khi duyệt: Up/Down di chuyển, Delete/Backspace xoá, Esc đóng menu trả về Consumed.
     ****/
    fn handle_event(&mut self, key: KeyEvent) -> EventResult {
        // 1. Khi ở trạng thái đóng: Chỉ nhận Enter hoặc Space để mở
        if !self.is_open {
            match key.code {
                KeyCode::Enter | KeyCode::Char(' ') => {
                    self.open();
                    return EventResult::Consumed;
                }
                _ => return EventResult::Ignored,
            }
        }

        let total_rows = 1 + self.items.len();

        // 2. Khi đang ở chế độ gõ nội dung (Inline Adding hoặc Editing)
        match &mut self.mode {
            EditMode::Adding { input, cursor } => {
                match key.code {
                    KeyCode::Enter => {
                        let trimmed = input.trim();
                        if !trimmed.is_empty() {
                            let can_add = self.max_items.map_or(true, |max| self.items.len() < max);
                            if can_add {
                                self.items.push(trimmed.to_string());
                                self.highlighted = self.items.len(); // Di chuyển tới item vừa thêm
                                self.adjust_scroll();
                            }
                        }
                        self.mode = EditMode::Browsing;
                        return EventResult::Consumed;
                    }
                    KeyCode::Esc => {
                        self.mode = EditMode::Browsing;
                        return EventResult::Consumed;
                    }
                    _ => {
                        if Self::handle_text_key(input, cursor, key) {
                            return EventResult::Consumed;
                        }
                    }
                }
                return EventResult::Consumed;
            }
            EditMode::Editing { index, input, cursor } => {
                let edit_idx = *index;
                match key.code {
                    KeyCode::Enter => {
                        let trimmed = input.trim();
                        if !trimmed.is_empty() && edit_idx < self.items.len() {
                            self.items[edit_idx] = trimmed.to_string();
                        }
                        self.mode = EditMode::Browsing;
                        return EventResult::Consumed;
                    }
                    KeyCode::Esc => {
                        self.mode = EditMode::Browsing;
                        return EventResult::Consumed;
                    }
                    _ => {
                        if Self::handle_text_key(input, cursor, key) {
                            return EventResult::Consumed;
                        }
                    }
                }
                return EventResult::Consumed;
            }
            EditMode::Browsing => {}
        }

        // 3. Khi đang ở chế độ duyệt danh sách (Browsing)
        match key.code {
            KeyCode::Up => {
                self.highlighted = self.highlighted.saturating_sub(1);
                self.adjust_scroll();
                EventResult::Consumed
            }
            KeyCode::Down => {
                if total_rows > 0 {
                    self.highlighted = (self.highlighted + 1).min(total_rows - 1);
                    self.adjust_scroll();
                }
                EventResult::Consumed
            }
            KeyCode::Home => {
                self.highlighted = 0;
                self.adjust_scroll();
                EventResult::Consumed
            }
            KeyCode::End => {
                if total_rows > 0 {
                    self.highlighted = total_rows - 1;
                    self.adjust_scroll();
                }
                EventResult::Consumed
            }
            KeyCode::PageUp => {
                self.highlighted = self.highlighted.saturating_sub(self.max_visible);
                self.adjust_scroll();
                EventResult::Consumed
            }
            KeyCode::PageDown => {
                if total_rows > 0 {
                    self.highlighted = (self.highlighted + self.max_visible).min(total_rows - 1);
                    self.adjust_scroll();
                }
                EventResult::Consumed
            }
            KeyCode::Enter => {
                if self.highlighted == 0 {
                    // Kích hoạt thêm mới nếu chưa đầy
                    let can_add = self.max_items.map_or(true, |max| self.items.len() < max);
                    if can_add {
                        self.mode = EditMode::Adding {
                            input: String::new(),
                            cursor: 0,
                        };
                    }
                } else {
                    // Kích hoạt sửa option
                    let item_idx = self.highlighted - 1;
                    if item_idx < self.items.len() {
                        let text = self.items[item_idx].clone();
                        let cursor = text.chars().count();
                        self.mode = EditMode::Editing {
                            index: item_idx,
                            input: text,
                            cursor,
                        };
                    }
                }
                EventResult::Consumed
            }
            KeyCode::Right => {
                // Nhấn mũi tên phải tại option để sửa nhanh
                if self.highlighted > 0 {
                    let item_idx = self.highlighted - 1;
                    if item_idx < self.items.len() {
                        let text = self.items[item_idx].clone();
                        let cursor = text.chars().count();
                        self.mode = EditMode::Editing {
                            index: item_idx,
                            input: text,
                            cursor,
                        };
                    }
                }
                EventResult::Consumed
            }
            KeyCode::Delete | KeyCode::Backspace => {
                // Xoá option đang chọn ngay lập tức
                if self.highlighted > 0 {
                    let item_idx = self.highlighted - 1;
                    if item_idx < self.items.len() {
                        self.items.remove(item_idx);
                        let new_total = 1 + self.items.len();
                        if self.highlighted >= new_total {
                            self.highlighted = new_total.saturating_sub(1);
                        }
                        self.adjust_scroll();
                    }
                }
                EventResult::Consumed
            }
            KeyCode::Esc | KeyCode::Left => {
                // Đóng menu và trở về form bên ngoài
                self.close();
                EventResult::Consumed
            }
            _ => EventResult::Ignored,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_editable_list_creation_and_value() {
        let widget = EditableListWidget::new("tags", "Tags", vec!["Rust", "TUI"]);
        assert_eq!(widget.value(), FormValue::List(vec!["Rust".into(), "TUI".into()]));
        assert_eq!(widget.items.len(), 2);
    }

    #[test]
    fn test_editable_list_add_item_workflow() {
        let mut widget = EditableListWidget::new("tags", "Tags", Vec::<String>::new());
        assert_eq!(widget.items.len(), 0);

        // 1. Mở widget
        let _ = widget.handle_event(KeyEvent::from(KeyCode::Enter));
        assert!(widget.is_open());
        assert_eq!(widget.highlighted, 0);

        // 2. Nhấn Enter tại "+ Thêm mới."
        let _ = widget.handle_event(KeyEvent::from(KeyCode::Enter));
        assert!(matches!(widget.mode, EditMode::Adding { .. }));

        // 3. Gõ chữ "Backend"
        for c in "Backend".chars() {
            let _ = widget.handle_event(KeyEvent::from(KeyCode::Char(c)));
        }

        // 4. Nhấn Enter để lưu
        let _ = widget.handle_event(KeyEvent::from(KeyCode::Enter));
        assert_eq!(widget.items, vec!["Backend".to_string()]);
        assert_eq!(widget.mode, EditMode::Browsing);
        assert_eq!(widget.highlighted, 1); // Đã chuyển focus tới item 1 vừa thêm
    }

    #[test]
    fn test_editable_list_edit_item_workflow() {
        let mut widget = EditableListWidget::new("tags", "Tags", vec!["OldName"]);
        widget.open();
        widget.highlighted = 1; // Chọn OldName

        // Nhấn Enter để sửa
        let _ = widget.handle_event(KeyEvent::from(KeyCode::Enter));
        assert!(matches!(widget.mode, EditMode::Editing { index: 0, .. }));

        // Xoá ký tự 'e' cuối bằng Backspace và gõ chữ mới
        let _ = widget.handle_event(KeyEvent::from(KeyCode::Backspace));
        let _ = widget.handle_event(KeyEvent::from(KeyCode::Char('!')));

        // Enter để lưu
        let _ = widget.handle_event(KeyEvent::from(KeyCode::Enter));
        assert_eq!(widget.items, vec!["OldNam!".to_string()]);
        assert_eq!(widget.mode, EditMode::Browsing);
    }

    #[test]
    fn test_editable_list_delete_item_workflow() {
        let mut widget = EditableListWidget::new("tags", "Tags", vec!["Item1", "Item2"]);
        widget.open();
        widget.highlighted = 1; // Chọn Item1

        // Nhấn Delete
        let _ = widget.handle_event(KeyEvent::from(KeyCode::Delete));
        assert_eq!(widget.items, vec!["Item2".to_string()]);

        // Nhấn Backspace xoá nốt Item2
        let _ = widget.handle_event(KeyEvent::from(KeyCode::Backspace));
        assert!(widget.items.is_empty());
        assert_eq!(widget.highlighted, 0); // Quay về "+ Thêm mới."
    }

    #[test]
    fn test_editable_list_esc_key_hierarchy() {
        let mut widget = EditableListWidget::new("tags", "Tags", vec!["Item1"]);
        widget.open();
        widget.highlighted = 1;

        // 1. Đang sửa text -> Esc chỉ huỷ chế độ sửa, menu vẫn mở
        let _ = widget.handle_event(KeyEvent::from(KeyCode::Enter));
        assert!(matches!(widget.mode, EditMode::Editing { .. }));
        let res = widget.handle_event(KeyEvent::from(KeyCode::Esc));
        assert_eq!(res, EventResult::Consumed);
        assert!(widget.is_open());
        assert_eq!(widget.mode, EditMode::Browsing);

        // 2. Đang duyệt -> Esc đóng menu hoàn toàn
        let res2 = widget.handle_event(KeyEvent::from(KeyCode::Esc));
        assert_eq!(res2, EventResult::Consumed);
        assert!(!widget.is_open());
    }

    #[test]
    fn test_render_input_spans_block_cursor() {
        // 1. Khi rỗng: hiển thị đúng [ █ ]
        let spans_empty = render_input_spans("", 0, 20);
        assert_eq!(spans_empty.len(), 1);
        assert_eq!(spans_empty[0].content, "█");

        // 2. Khi con trỏ ở cuối: [ abc█ ]
        let spans_end = render_input_spans("abc", 3, 20);
        assert_eq!(spans_end.len(), 2);
        assert_eq!(spans_end[0].content, "abc");
        assert_eq!(spans_end[1].content, "█");

        // 3. Khi con trỏ ở giữa: [ a█bc ]
        let spans_mid = render_input_spans("abc", 1, 20);
        assert_eq!(spans_mid.len(), 3);
        assert_eq!(spans_mid[0].content, "a");
        assert_eq!(spans_mid[1].content, "█");
        assert_eq!(spans_mid[2].content, "bc");
    }
}
