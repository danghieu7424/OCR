// --- PHÂN ĐOẠN: DEMO TEST TRÌNH DIỄN BỘ BIỂU TƯỢNG ICONS VÀ CÁC WIDGETS ---

use cli_gui_form::{
    ButtonWidget, CheckboxWidget, EventResult, FormManager, FormValue, FormWidget, Icons,
    InputMode, InputWidget, RadioWidget, SpinnerType, TaskWidget,
};
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
 * Chức năng: Cho phép chia sẻ TaskWidget giữa FormManager và vòng lặp render/tick.
 ****/
struct TaskShared(Arc<Mutex<TaskWidget>>);

impl FormWidget for TaskShared {
    fn id(&self) -> &str {
        "shared_task"
    }

    fn render(&self, area: ratatui::layout::Rect, frame: &mut ratatui::Frame) {
        if let Ok(w) = self.0.lock() {
            w.render(area, frame);
        }
    }

    fn handle_event(&mut self, key: crossterm::event::KeyEvent) -> EventResult {
        if let Ok(mut w) = self.0.lock() {
            w.handle_event(key)
        } else {
            EventResult::Ignored
        }
    }

    fn focus(&mut self) {
        if let Ok(mut w) = self.0.lock() {
            w.focus();
        }
    }

    fn blur(&mut self) {
        if let Ok(mut w) = self.0.lock() {
            w.blur();
        }
    }

    fn is_focused(&self) -> bool {
        self.0.lock().map(|w| w.is_focused()).unwrap_or(false)
    }

    fn preferred_height(&self) -> u16 {
        self.0.lock().map(|w| w.preferred_height()).unwrap_or(3)
    }
}

fn main() -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut form = FormManager::new();

    // 1. Text Inputs
    form.add_widget(Box::new(InputWidget::new("task_name", "Tên tiến trình", InputMode::Text)));

    // 2. Checkboxes (Tự động áp dụng Icons::CHECKBOX_ON '☑' và Icons::CHECKBOX_OFF '☐')
    form.add_widget(Box::new(CheckboxWidget::new("auto_retry", "Tự động thử lại khi lỗi", true)));
    form.add_widget(Box::new(CheckboxWidget::new("send_notify", "Gửi thông báo hoàn tất", false)));

    // 3. Radio Options (Tự động áp dụng Icons::RADIO_ON '●' và Icons::RADIO_OFF '○')
    form.add_widget(Box::new(RadioWidget::new(
        "log_level",
        "Mức độ Log",
        vec!["Debug".into(), "Info".into(), "Warning".into(), "Error".into()],
    )));

    // 4. TaskWidget với SpinnerType::Pulse (Icons::THINKING_FRAMES)
    let ai_task = Arc::new(Mutex::new(
        TaskWidget::new_loading(
            "ai_worker",
            "Trạng thái AI",
            "Đang kết nối worker backend...",
            Icons::color_build(), // Tím Magenta
        )
        .with_spinner_type(SpinnerType::Pulse),
    ));
    form.add_widget(Box::new(TaskShared(Arc::clone(&ai_task))));

    // 5. Buttons với Icons từ bảng chuẩn: RUN (▶), STOP (■), SUCCESS (✔)
    form.add_widget(Box::new(
        ButtonWidget::new("btn_start", "BẮT ĐẦU CHẠY", Icons::color_run(), Color::Black)
            .with_icon(Icons::RUN),
    ));
    form.add_widget(Box::new(
        ButtonWidget::new("btn_submit", "LƯU CẤU HÌNH", Icons::color_success(), Color::Black)
            .with_icon(Icons::SUCCESS),
    ));

    let frame_interval = Duration::from_millis(16); // ~60 FPS
    let mut last_tick = Instant::now();

    loop {
        // Cập nhật animation pulse 100ms
        if last_tick.elapsed() >= Duration::from_millis(100) {
            if let Ok(mut w) = ai_task.lock() {
                w.tick();
            }
            last_tick = Instant::now();
        }

        terminal.draw(|f| {
            form.render(f.area(), f);
        })?;

        if event::poll(frame_interval)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    let is_ctrl_c = key.modifiers.contains(KeyModifiers::CONTROL)
                        && key.code == KeyCode::Char('c');
                    if key.code == KeyCode::Esc || is_ctrl_c {
                        break;
                    }

                    let res = form.handle_event(key);
                    if res == EventResult::Submitted {
                        break;
                    }
                }
            }
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    println!("\n=== KẾT THÚC TEST FORM & ICONS ===");
    for (k, v) in form.get_values() {
        match v {
            FormValue::Text(t) => println!("- {}: \"{}\"", k, t),
            FormValue::Bool(b) => println!("- {}: {}", k, b),
            FormValue::Select(idx, name) => println!("- {}: [{}] {}", k, idx, name),
            FormValue::List(items) => println!("- {}: {:?}", k, items),
            FormValue::None => {}
        }
    }

    Ok(())
}
