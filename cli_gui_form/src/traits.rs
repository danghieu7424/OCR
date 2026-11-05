// --- PHÂN ĐOẠN: FORM WIDGET TRAIT VỚI CURSOR, VALUE VÀ VIEWPORT ---

use crossterm::event::KeyEvent;
use ratatui::{layout::Rect, Frame};

#[derive(Debug, PartialEq, Eq)]
pub enum EventResult {
    Consumed,
    Ignored,
    Submitted,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FormValue {
    Text(String),
    Bool(bool),
    Select(usize, String),
    List(Vec<String>),
    None,
}

pub trait FormWidget: Send {
    /// Định danh duy nhất để lấy giá trị form khi submit
    fn id(&self) -> &str;

    /// Trích xuất giá trị hiện tại của control
    fn value(&self) -> FormValue {
        FormValue::None
    }

    fn render(&self, area: Rect, frame: &mut Frame);

    fn handle_event(&mut self, key: KeyEvent) -> EventResult;

    fn focus(&mut self);

    fn blur(&mut self);

    fn is_focused(&self) -> bool;

    fn preferred_height(&self) -> u16;

    /// Tọa độ con trỏ (x, y) nếu widget có con trỏ nhấp nháy trong terminal
    fn cursor_position(&self, _area: Rect) -> Option<(u16, u16)> {
        None
    }

    /// Vẽ lớp phủ popup/dropdown trên đỉnh nếu widget đang mở modal
    fn render_overlay(&self, _area: Rect, _frame: &mut Frame) {}
}