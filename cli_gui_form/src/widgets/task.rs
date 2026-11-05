// --- PHÂN ĐOẠN: TASK WIDGET TOÀN DIỆN (THAY THẾ HOÀN TOÀN CHO LOADING & PROGRESS) ---

use crate::traits::{EventResult, FormWidget};
use crossterm::event::KeyEvent;
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

const SPINNER_FRAMES_DOTS: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
const SPINNER_FRAMES_BRAILLE_6: &[&str] = crate::icons::Icons::SPINNER_BRAILLE_6_FRAMES;
const SPINNER_FRAMES_PULSE: &[&str] = &["·", "•", "●", "•", "·", " "];
const SPINNER_FRAMES_MOON: &[&str] = crate::icons::Icons::SPINNER_MOON_FRAMES;
const SPINNER_FRAMES_ARC: &[&str] = crate::icons::Icons::SPINNER_ARC_FRAMES;

/****
 * Module: SpinnerType
 * Chức năng: Định nghĩa kiểu hoạt họa spinner trong TaskState::Loading.
 * - Dots: Vòng xoay Braille 10 khung hình mặc định ("⠋"..."⠏")
 * - Braille6: Vòng xoay Braille 6 khung viền kinh điển ("⠋", "⠙", "⠸", "⠴", "⠦", "⠇")
 * - Pulse: Hiệu ứng chấm nhịp đập / Thinking ("·", "•", "●", "•", "·", " ")
 * - Moon: Vòng xoay 4 pha bán cầu xuôi chiều kim đồng hồ ("◐", "◒", "◑", "◓")
 * - Arc: Vòng xoay cung phần tư tròn 4 khung hình ("◜", "◝", "◞", "◟")
 ****/
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SpinnerType {
    #[default]
    Dots,
    Braille6,
    Pulse,
    Moon,
    Arc,
}

impl SpinnerType {
    pub fn frames(&self) -> &'static [&'static str] {
        match self {
            SpinnerType::Dots => SPINNER_FRAMES_DOTS,
            SpinnerType::Braille6 => SPINNER_FRAMES_BRAILLE_6,
            SpinnerType::Pulse => SPINNER_FRAMES_PULSE,
            SpinnerType::Moon => SPINNER_FRAMES_MOON,
            SpinnerType::Arc => SPINNER_FRAMES_ARC,
        }
    }
}

/****
 * Module: ProgressStyle
 * Chức năng: Định nghĩa kiểu ký tự hiển thị thanh tiến trình (Progress Bar).
 * - Line: Thanh vạch mảnh chuẩn tối giản Vercel ("━" / "─")
 * - Parallelogram: Hình bình hành xiên phong cách Hiện đại / Cyberpunk ("▰" / "▱")
 ****/
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProgressStyle {
    #[default]
    Line,
    Parallelogram,
}

impl ProgressStyle {
    #[inline]
    pub const fn chars(&self) -> (&'static str, &'static str) {
        match self {
            ProgressStyle::Line => crate::Icons::PROGRESS_CHARS_LINE,
            ProgressStyle::Parallelogram => crate::Icons::PROGRESS_CHARS_PARALLELOGRAM,
        }
    }
}

pub enum TaskState {
    Loading {
        message: String,
        frame_idx: usize,
        bouncing_pos: usize,
        moving_forward: bool,
    },
    Running {
        current: usize,
        total: usize,
        unit: String,
        duration: String,
        status: String,
    },
}

pub struct TaskWidget {
    pub id: String,
    pub label: String,
    pub state: TaskState,
    pub color: Color,
    pub bar_width: usize,
    pub spinner_type: SpinnerType,
    pub progress_style: ProgressStyle,
    focused: bool,
}

impl TaskWidget {
    /// 1. Khởi tạo ở chế độ thuần Loading (thay thế hoàn toàn cho LoadingWidget)
    pub fn new_loading(
        id: impl Into<String>,
        label: impl Into<String>,
        message: impl Into<String>,
        color: Color,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            state: TaskState::Loading {
                message: message.into(),
                frame_idx: 0,
                bouncing_pos: 0,
                moving_forward: true,
            },
            color,
            bar_width: 25,
            spinner_type: SpinnerType::Dots,
            progress_style: ProgressStyle::Line,
            focused: false,
        }
    }

    /// 2. Khởi tạo ở chế độ thuần Progress (thay thế hoàn toàn cho ProgressWidget)
    pub fn new_progress(
        id: impl Into<String>,
        label: impl Into<String>,
        current: usize,
        total: usize,
        unit: impl Into<String>,
        duration: impl Into<String>,
        status: impl Into<String>,
        color: Color,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            state: TaskState::Running {
                current,
                total: total.max(1),
                unit: unit.into(),
                duration: duration.into(),
                status: status.into(),
            },
            color,
            bar_width: 25,
            spinner_type: SpinnerType::Dots,
            progress_style: ProgressStyle::Line,
            focused: false,
        }
    }

    /// Cho phép cấu hình kiểu animation của spinner (Dots, Pulse hoặc Moon)
    pub fn with_spinner_type(mut self, spinner_type: SpinnerType) -> Self {
        self.spinner_type = spinner_type;
        self
    }

    /// Cho phép cấu hình phong cách hiển thị thanh tiến trình (Line, Parallelogram, Rectangle, Square)
    pub fn with_progress_style(mut self, progress_style: ProgressStyle) -> Self {
        self.progress_style = progress_style;
        self
    }

    /// Chuyển đổi trạng thái từ Loading sang Running
    pub fn switch_to_progress(
        &mut self,
        label: impl Into<String>,
        total: usize,
        unit: impl Into<String>,
        duration: impl Into<String>,
        status: impl Into<String>,
    ) {
        self.label = label.into();
        self.state = TaskState::Running {
            current: 0,
            total: total.max(1),
            unit: unit.into(),
            duration: duration.into(),
            status: status.into(),
        };
    }

    /// Cập nhật tiến trình khi đang ở pha Running
    pub fn update_progress(&mut self, current: usize, duration: impl Into<String>) {
        if let TaskState::Running {
            current: c,
            total,
            duration: d,
            ..
        } = &mut self.state
        {
            *c = current.min(*total);
            *d = duration.into();
        }
    }

    /// Nhịp animation cho spinner ở pha Loading
    pub fn tick(&mut self) {
        let frames = self.spinner_type.frames();
        if let TaskState::Loading {
            frame_idx,
            bouncing_pos,
            moving_forward,
            ..
        } = &mut self.state
        {
            *frame_idx = (*frame_idx + 1) % frames.len();
            let block_size = 5;
            if self.bar_width > block_size {
                let max_pos = self.bar_width - block_size;
                if *moving_forward {
                    if *bouncing_pos >= max_pos {
                        *moving_forward = false;
                    } else {
                        *bouncing_pos += 1;
                    }
                } else if *bouncing_pos == 0 {
                    *moving_forward = true;
                } else {
                    *bouncing_pos -= 1;
                }
            }
        }
    }
}

impl FormWidget for TaskWidget {
    fn id(&self) -> &str {
        &self.id
    }

    fn render(&self, area: Rect, frame: &mut Frame) {
        let label_style = if self.focused {
            Style::default().fg(crate::Theme::PRIMARY).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(crate::Theme::SECONDARY)
        };

        match &self.state {
            TaskState::Loading {
                message,
                frame_idx,
                bouncing_pos,
                ..
            } => {
                let frames = self.spinner_type.frames();
                let spinner_char = frames[*frame_idx % frames.len()];
                let block_size = 5;
                let left_empty = *bouncing_pos;
                let active_len = block_size.min(self.bar_width.saturating_sub(left_empty));
                let right_empty = self.bar_width.saturating_sub(left_empty + active_len);

                // Theo DESIGN.md mục 6: 2-space indent, Accent/Primary cho tiến trình, Muted cho nét trống
                let (filled_char, empty_char) = self.progress_style.chars();
                let line1 = Line::from(vec![
                    Span::styled("  ", Style::default()),
                    Span::styled(format!("{} ", spinner_char), Style::default().fg(self.color).add_modifier(Modifier::BOLD)),
                    Span::styled(format!("{}: ", self.label), label_style),
                    Span::styled(empty_char.repeat(left_empty), Style::default().fg(crate::Theme::MUTED)),
                    Span::styled(filled_char.repeat(active_len), Style::default().fg(self.color).add_modifier(Modifier::BOLD)),
                    Span::styled(empty_char.repeat(right_empty), Style::default().fg(crate::Theme::MUTED)),
                ]);

                // DESIGN.md: Tránh italic, dùng Muted / Secondary với Icons::BRANCH (thụt lề 2 spaces + 2-cell branch icon)
                let line2 = Line::from(vec![
                    Span::styled(format!("    {}", crate::Icons::BRANCH), Style::default().fg(crate::Theme::MUTED)),
                    Span::styled(message, Style::default().fg(crate::Theme::SECONDARY)),
                ]);

                frame.render_widget(Paragraph::new(vec![line1, line2]), area);
            }
            TaskState::Running {
                current,
                total,
                unit,
                duration,
                status,
            } => {
                let ratio = (*current as f32 / *total as f32).clamp(0.0, 1.0);
                let percent = (ratio * 100.0).round() as u8;

                let filled_len = ((self.bar_width as f32) * ratio).round() as usize;
                let empty_len = self.bar_width.saturating_sub(filled_len);

                let metric_text = format!(" [{}/{} {} ({})]", current, total, unit, duration);

                let (filled_char, empty_char) = self.progress_style.chars();
                let line1 = Line::from(vec![
                    Span::styled("    ", Style::default()), // Căn lề thụt đầu dòng 4 spaces (2 indent + 2 icon width)
                    Span::styled(format!("{}: ", self.label), label_style),
                    Span::styled(filled_char.repeat(filled_len), Style::default().fg(crate::Theme::ACCENT).add_modifier(Modifier::BOLD)),
                    Span::styled(empty_char.repeat(empty_len), Style::default().fg(crate::Theme::MUTED)),
                    Span::styled(format!(" {:>3}%", percent), Style::default().fg(crate::Theme::PRIMARY).add_modifier(Modifier::BOLD)),
                    Span::styled(metric_text, Style::default().fg(crate::Theme::SECONDARY)),
                ]);

                let line2 = Line::from(vec![
                    Span::styled(format!("    {}", crate::Icons::BRANCH), Style::default().fg(crate::Theme::MUTED)),
                    Span::styled(status, Style::default().fg(crate::Theme::SECONDARY)),
                ]);

                frame.render_widget(Paragraph::new(vec![line1, line2]), area);
            }
        }
    }

    fn handle_event(&mut self, _key: KeyEvent) -> EventResult {
        EventResult::Ignored
    }

    fn focus(&mut self) { self.focused = true; }
    fn blur(&mut self) { self.focused = false; }
    fn is_focused(&self) -> bool { self.focused }
    fn preferred_height(&self) -> u16 { 3 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spinner_type_frames() {
        assert_eq!(SpinnerType::Dots.frames()[0], "⠋");
        assert_eq!(SpinnerType::Braille6.frames().len(), 6);
        assert_eq!(SpinnerType::Braille6.frames(), &["⠋", "⠙", "⠸", "⠴", "⠦", "⠇"]);
        assert_eq!(SpinnerType::Pulse.frames(), &["·", "•", "●", "•", "·", " "]);
        assert_eq!(SpinnerType::Moon.frames(), &["◐", "◒", "◑", "◓"]);
        assert_eq!(SpinnerType::Arc.frames(), &["◜", "◝", "◞", "◟"]);
        assert_eq!(SpinnerType::Arc.frames().len(), 4);
    }

    #[test]
    fn test_progress_style_chars() {
        assert_eq!(ProgressStyle::Line.chars(), ("━", "─"));
        assert_eq!(ProgressStyle::Parallelogram.chars(), ("▰", "▱"));
    }

    #[test]
    fn test_task_loading_tick_with_pulse() {
        let mut widget = TaskWidget::new_loading("task", "Thinking", "Please wait...", Color::Magenta)
            .with_spinner_type(SpinnerType::Pulse);

        assert_eq!(widget.spinner_type, SpinnerType::Pulse);
        if let TaskState::Loading { frame_idx, .. } = widget.state {
            assert_eq!(frame_idx, 0);
        }

        widget.tick();
        if let TaskState::Loading { frame_idx, .. } = widget.state {
            assert_eq!(frame_idx, 1);
        }
    }

    #[test]
    fn test_task_loading_tick_with_moon() {
        let mut widget = TaskWidget::new_loading("task_moon", "Syncing", "Moon spinner...", Color::Cyan)
            .with_spinner_type(SpinnerType::Moon);

        assert_eq!(widget.spinner_type, SpinnerType::Moon);
        if let TaskState::Loading { frame_idx, .. } = widget.state {
            assert_eq!(frame_idx, 0);
        }

        widget.tick();
        if let TaskState::Loading { frame_idx, .. } = widget.state {
            assert_eq!(frame_idx, 1);
        }
    }

    #[test]
    fn test_task_progress_style_builder() {
        let widget = TaskWidget::new_progress("task_p", "WASM", 10, 20, "kb", "1s", "Compiling", Color::Green)
            .with_progress_style(ProgressStyle::Parallelogram);

        assert_eq!(widget.progress_style, ProgressStyle::Parallelogram);
    }
}