use cli_gui_form::{
    ButtonWidget, CheckboxWidget, EventResult, FormManager, FormValue, FormWidget, InputMode,
    InputWidget, RadioWidget, TaskWidget,
};
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, style::Color, Terminal};
use std::{
    io::{self, stdout},
    sync::{
        mpsc::{channel, Receiver, Sender},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

enum WorkerMessage {
    ModelLoaded,
    ProgressUpdate { current: usize, duration: String },
}

fn main() -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut form = FormManager::new();

    form.add_widget(Box::new(InputWidget::new("user", "Username", InputMode::Text)));
    form.add_widget(Box::new(InputWidget::new("pass", "Password", InputMode::Password)));
    form.add_widget(Box::new(CheckboxWidget::new("remember", "Remember Me", false)));
    form.add_widget(Box::new(RadioWidget::new(
        "env",
        "Environment",
        vec!["Dev".into(), "Staging".into(), "Prod".into()],
    )));

    form.add_widget(Box::new(TaskWidget::new_progress(
        "static_progress",
        "TTS Generator",
        667,
        667,
        "items",
        "0s",
        "Hoàn tất sinh audio các câu thoại",
        Color::Green,
    )));

    let dynamic_task = Arc::new(Mutex::new(TaskWidget::new_loading(
        "ai_task",
        "Processing",
        "Đang nạp mô hình trọng số từ luồng background...",
        Color::Magenta,
    )));
    form.add_widget(Box::new(TaskShared(Arc::clone(&dynamic_task))));

    form.add_widget(Box::new(ButtonWidget::new("submit", "SUBMIT", Color::Blue, Color::White)));

    let (tx, rx): (Sender<WorkerMessage>, Receiver<WorkerMessage>) = channel();
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(2500));
        let _ = tx.send(WorkerMessage::ModelLoaded);

        for chunk in 1..=134 {
            thread::sleep(Duration::from_millis(30));
            let _ = tx.send(WorkerMessage::ProgressUpdate {
                current: chunk,
                duration: format!("{}s", chunk / 10),
            });
        }
    });

    let target_frame_duration = Duration::from_millis(16);
    let mut last_tick = Instant::now();
    let mut submitted_values = None;

    loop {
        while let Ok(msg) = rx.try_recv() {
            let mut t = dynamic_task.lock().unwrap();
            match msg {
                WorkerMessage::ModelLoaded => {
                    t.color = Color::Cyan;
                    t.switch_to_progress(
                        "Separation",
                        134,
                        "chunks",
                        "0s",
                        "MDX-Net Native Inference",
                    );
                }
                WorkerMessage::ProgressUpdate { current, duration } => {
                    t.update_progress(current, duration);
                }
            }
        }

        terminal.draw(|f| {
            form.render(f.area(), f);
        })?;

        if last_tick.elapsed() >= Duration::from_millis(40) {
            if let Ok(mut t) = dynamic_task.lock() {
                t.tick();
            }
            last_tick = Instant::now();
        }

        if event::poll(target_frame_duration)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    let is_ctrl_c = key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c');
                    if key.code == KeyCode::Esc || is_ctrl_c {
                        break;
                    }

                    let res = form.handle_event(key);
                    if res == EventResult::Submitted {
                        submitted_values = Some(form.get_values());
                        break;
                    }
                }
            }
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    if let Some(values) = submitted_values {
        println!("\n=== DỮ LIỆU ĐÃ SUBMIT ===");
        if let Some(FormValue::Text(u)) = values.get("user") {
            println!("Username: {}", u);
        }
        if let Some(FormValue::Text(p)) = values.get("pass") {
            println!("Password: {}", "*".repeat(p.len()));
        }
        if let Some(FormValue::Bool(r)) = values.get("remember") {
            println!("Remember: {}", r);
        }
        if let Some(FormValue::Select(idx, name)) = values.get("env") {
            println!("Environment: [{}] {}", idx, name);
        }
    }

    Ok(())
}

struct TaskShared(Arc<Mutex<TaskWidget>>);

impl FormWidget for TaskShared {
    fn id(&self) -> &str {
        "ai_task"
    }

    fn value(&self) -> FormValue {
        FormValue::None
    }

    fn render(&self, area: ratatui::layout::Rect, frame: &mut ratatui::Frame) {
        if let Ok(t) = self.0.lock() {
            t.render(area, frame);
        }
    }

    fn handle_event(&mut self, key: crossterm::event::KeyEvent) -> EventResult {
        if let Ok(mut t) = self.0.lock() {
            t.handle_event(key)
        } else {
            EventResult::Ignored
        }
    }

    fn focus(&mut self) {
        if let Ok(mut t) = self.0.lock() {
            t.focus();
        }
    }

    fn blur(&mut self) {
        if let Ok(mut t) = self.0.lock() {
            t.blur();
        }
    }

    fn is_focused(&self) -> bool {
        self.0.lock().map(|t| t.is_focused()).unwrap_or(false)
    }

    fn preferred_height(&self) -> u16 {
        self.0.lock().map(|t| t.preferred_height()).unwrap_or(3)
    }
}
