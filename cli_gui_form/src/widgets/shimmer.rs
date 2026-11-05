// --- PHÂN ĐOẠN: SHIMMER TEXT WIDGET VỚI RATATUI TRUECOLOR GRADIENT ---

use crate::traits::{EventResult, FormWidget};
use crossterm::event::KeyEvent;
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

/****
 * Module: ShimmerWidget
 * Chức năng: Render chuỗi văn bản với hiệu ứng dải sáng (shimmer wave) lướt qua theo nhịp tick.
 * Đầu vào: id, text, cấu hình màu sắc (base, highlight), wave_width, step.
 * Đầu ra: FormWidget render dòng chữ với Color::Rgb gradient trên từng ký tự.
 * Ranh giới bảo vệ: Khép kín trạng thái sóng (wave_pos) nội bộ; chỉ tính toán lại khi gọi tick().
 ****/
pub struct ShimmerWidget {
    pub id: String,
    pub text: String,
    pub color_base: (u8, u8, u8),
    pub color_highlight: (u8, u8, u8),
    pub wave_width: f32,
    pub step: f32,
    wave_pos: f32,
    focused: bool,
}

impl ShimmerWidget {
    /****
     * Khởi tạo ShimmerWidget với các tham số mặc định chuẩn theo nguyên mẫu
     ****/
    pub fn new(id: impl Into<String>, text: impl Into<String>) -> Self {
        // Mặc định: wave_width = 2.5 ký tự, step = 0.35 ký tự mỗi tick
        let wave_width = 2.5;
        Self {
            id: id.into(),
            text: text.into(),
            color_base: (90, 90, 90),          // Xám tối nền
            color_highlight: (235, 235, 235), // Trắng nhạt đỉnh sóng
            wave_width,
            step: 0.35,
            wave_pos: -wave_width,
            focused: false,
        }
    }

    /****
     * Cho phép tùy biến bảng màu base và highlight
     ****/
    pub fn with_colors(mut self, base: (u8, u8, u8), highlight: (u8, u8, u8)) -> Self {
        self.color_base = base;
        self.color_highlight = highlight;
        self
    }

    /****
     * Cho phép cấu hình độ rộng dải sóng và tốc độ quét
     ****/
    pub fn with_wave_config(mut self, wave_width: f32, step: f32) -> Self {
        self.wave_width = wave_width;
        self.step = step;
        self.wave_pos = -wave_width;
        self
    }

    /****
     * Nội suy tuyến tính kênh màu (Linear Interpolation)
     ****/
    #[inline]
    fn interpolate(start: u8, end: u8, factor: f32) -> u8 {
        // Factor được kẹp trong ngưỡng [0.0, 1.0] để tránh tràn giá trị u8
        let factor = factor.clamp(0.0, 1.0);
        ((start as f32) + (end as f32 - start as f32) * factor).round() as u8
    }

    /****
     * Tính toán màu RGB tại vị trí index ký tự dựa trên tâm sóng wave_pos
     ****/
    fn calculate_char_color(&self, char_idx: usize) -> (u8, u8, u8) {
        let distance = (char_idx as f32 - self.wave_pos).abs();

        if distance > self.wave_width {
            self.color_base
        } else {
            // Càng gần đỉnh sóng, factor càng tiến tới 1.0
            let factor = 1.0 - (distance / self.wave_width);
            (
                Self::interpolate(self.color_base.0, self.color_highlight.0, factor),
                Self::interpolate(self.color_base.1, self.color_highlight.1, factor),
                Self::interpolate(self.color_base.2, self.color_highlight.2, factor),
            )
        }
    }

    /****
     * Cập nhật bước sóng animation (gọi theo vòng lặp tick UI ~30-60 FPS)
     ****/
    pub fn tick(&mut self) {
        let text_len = self.text.chars().count();
        let start_pos = -self.wave_width;
        let end_pos = (text_len as f32) + self.wave_width;

        self.wave_pos += self.step;
        if self.wave_pos > end_pos {
            self.wave_pos = start_pos;
        }
    }
}

impl FormWidget for ShimmerWidget {
    fn id(&self) -> &str {
        &self.id
    }

    fn render(&self, area: Rect, frame: &mut Frame) {
        let spans: Vec<Span> = self
            .text
            .chars()
            .enumerate()
            .map(|(idx, ch)| {
                let (r, g, b) = self.calculate_char_color(idx);
                Span::styled(ch.to_string(), Style::default().fg(Color::Rgb(r, g, b)))
            })
            .collect();

        let line = Line::from(spans);
        frame.render_widget(Paragraph::new(line), area);
    }

    fn handle_event(&mut self, _key: KeyEvent) -> EventResult {
        // ShimmerWidget thuần hiển thị thông báo, bỏ qua phím
        EventResult::Ignored
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
        1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shimmer_interpolation_bounds() {
        assert_eq!(ShimmerWidget::interpolate(90, 235, 0.0), 90);
        assert_eq!(ShimmerWidget::interpolate(90, 235, 1.0), 235);
        assert_eq!(ShimmerWidget::interpolate(90, 235, 0.5), 163);
    }

    #[test]
    fn test_shimmer_tick_cycle() {
        let mut widget = ShimmerWidget::new("test", "Wait");
        let initial_pos = widget.wave_pos;
        widget.tick();
        assert!(widget.wave_pos > initial_pos);
    }
}
