// --- PHÂN ĐOẠN: EXAMPLE RUNNER CHO SHIMMER WIDGET ---

use cli_gui_form::{EventResult, FormManager, ShimmerWidget};
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::{
    io::{self, stdout},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

/****
 * Module: ShimmerShared
 * Chức năng: Bọc ShimmerWidget bên trong Arc<Mutex<_>> để cho phép FormManager render
 *            đồng thời Render Loop có thể gọi tick() cập nhật nhịp sóng liên tục.
 * Đầu vào: Arc<Mutex<ShimmerWidget>>
 * Đầu ra: Triển khai FormWidget delegate an toàn qua MutexGuard.
 ****/
struct ShimmerShared(Arc<Mutex<ShimmerWidget>>);

impl cli_gui_form::FormWidget for ShimmerShared {
    fn id(&self) -> &str {
        "status_shimmer"
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
        self.0.lock().map(|w| w.preferred_height()).unwrap_or(1)
    }
}

fn main() -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut form = FormManager::new();

    // 1. Khởi tạo ShimmerWidget và đưa vào Arc<Mutex>
    let shimmer_arc = Arc::new(Mutex::new(
        ShimmerWidget::new("status_shimmer", "Waiting for server response...")
            .with_colors((90, 90, 90), (240, 240, 240))
            .with_wave_config(3.0, 0.4),
    ));

    form.add_widget(Box::new(ShimmerShared(Arc::clone(&shimmer_arc))));

    let frame_interval = Duration::from_millis(33); // ~30 FPS
    let mut last_tick = Instant::now();

    loop {
        // Cập nhật nhịp sóng gradient
        if last_tick.elapsed() >= Duration::from_millis(33) {
            if let Ok(mut w) = shimmer_arc.lock() {
                w.tick();
            }
            last_tick = Instant::now();
        }

        // Vẽ màn hình Ratatui
        terminal.draw(|f| {
            form.render(f.area(), f);
        })?;

        // Lắng nghe sự kiện phím (thoát bằng ESC hoặc Ctrl+C / Enter)
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
