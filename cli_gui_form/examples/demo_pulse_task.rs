// --- PHÂN ĐOẠN: DEMO TASK VỚI SPINNER PULSE (THINKING DOTS) VÀ BRAILLE ---

use cli_gui_form::{EventResult, FormManager, FormWidget, SpinnerType, TaskWidget};
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, style::Color, Terminal};
use std::{
    io::{self, stdout},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

/****
 * Module: TaskShared
 * Chức năng: Delegate an toàn TaskWidget qua MutexGuard để FormManager vẽ frame
 *            trong khi Render Loop tick animation độc lập.
 ****/
struct TaskShared {
    id: String,
    inner: Arc<Mutex<TaskWidget>>,
}

impl TaskShared {
    fn new(id: impl Into<String>, inner: Arc<Mutex<TaskWidget>>) -> Self {
        Self {
            id: id.into(),
            inner,
        }
    }
}

impl FormWidget for TaskShared {
    fn id(&self) -> &str {
        &self.id
    }

    fn render(&self, area: ratatui::layout::Rect, frame: &mut ratatui::Frame) {
        if let Ok(w) = self.inner.lock() {
            w.render(area, frame);
        }
    }

    fn handle_event(&mut self, key: crossterm::event::KeyEvent) -> EventResult {
        if let Ok(mut w) = self.inner.lock() {
            w.handle_event(key)
        } else {
            EventResult::Ignored
        }
    }

    fn focus(&mut self) {
        if let Ok(mut w) = self.inner.lock() {
            w.focus();
        }
    }

    fn blur(&mut self) {
        if let Ok(mut w) = self.inner.lock() {
            w.blur();
        }
    }

    fn is_focused(&self) -> bool {
        self.inner.lock().map(|w| w.is_focused()).unwrap_or(false)
    }

    fn preferred_height(&self) -> u16 {
        self.inner.lock().map(|w| w.preferred_height()).unwrap_or(3)
    }
}

fn main() -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut form = FormManager::new();

    // 1. TaskWidget với SpinnerType::Pulse (Thinking dots: · -> • -> ● -> • -> · -> trống)
    let thinking_task = Arc::new(Mutex::new(
        TaskWidget::new_loading(
            "ai_thinking",
            "Thinking",
            "AI đang suy nghĩ và tính toán vector...",
            Color::Magenta,
        )
        .with_spinner_type(SpinnerType::Pulse),
    ));

    // 2. TaskWidget với SpinnerType::Dots (Vòng xoay Braille mặc định)
    let default_task = Arc::new(Mutex::new(
        TaskWidget::new_loading(
            "data_loading",
            "Loading",
            "Đang tải tài nguyên mô hình nền...",
            Color::Cyan,
        )
        .with_spinner_type(SpinnerType::Dots),
    ));

    form.add_widget(Box::new(TaskShared::new("ai_thinking", Arc::clone(&thinking_task))));
    form.add_widget(Box::new(TaskShared::new("data_loading", Arc::clone(&default_task))));

    let frame_interval = Duration::from_millis(16); // ~60 FPS
    let mut last_pulse_tick = Instant::now();
    let mut last_dots_tick = Instant::now();

    loop {
        let now = Instant::now();

        // Chu kỳ nhịp đập Pulse (~150ms theo đặc tả DESIGN.md)
        if now.duration_since(last_pulse_tick) >= Duration::from_millis(100) {
            if let Ok(mut w) = thinking_task.lock() {
                w.tick();
            }
            last_pulse_tick = now;
        }

        // Chu kỳ xoay Braille (~80ms mượt mà)
        if now.duration_since(last_dots_tick) >= Duration::from_millis(80) {
            if let Ok(mut w) = default_task.lock() {
                w.tick();
            }
            last_dots_tick = now;
        }

        // Vẽ màn hình Ratatui
        terminal.draw(|f| {
            form.render(f.area(), f);
        })?;

        // Bắt sự kiện phím thoát (ESC, Enter, Ctrl+C)
        if event::poll(frame_interval)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    let is_ctrl_c = key.modifiers.contains(KeyModifiers::CONTROL)
                        && key.code == KeyCode::Char('c');
                    if key.code == KeyCode::Esc || key.code == KeyCode::Enter || is_ctrl_c {
                        break;
                    }
                }
            }
        }
    }

    // Khôi phục terminal trạng thái ban đầu
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}
