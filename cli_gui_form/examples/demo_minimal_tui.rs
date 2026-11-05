// --- PHÂN ĐOẠN: DEMO MINIMAL TUI VỚI CÁC THÀNH PHẦN THEO CHUẨN DESIGN.MD ---

use cli_gui_form::{
    ButtonWidget, CheckboxWidget, EventResult, FormManager, FormValue, FormWidget,
    InputMode, InputWidget, RadioWidget, SpinnerType, TaskWidget, Theme,
};
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
    Terminal,
};
use std::{
    io::{self, stdout},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

/****
 * Module: TaskShared
 * Chức năng: Cho phép chia sẻ TaskWidget giữa FormManager và luồng cập nhật animation.
 ****/
struct TaskShared(Arc<Mutex<TaskWidget>>);

impl FormWidget for TaskShared {
    fn id(&self) -> &str {
        "shared_task"
    }

    fn render(&self, area: Rect, frame: &mut ratatui::Frame) {
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

    // 1. Text Inputs (Single-line border, Accent on active)
    form.add_widget(Box::new(InputWidget::new(
        "service_name",
        "Service Name",
        InputMode::Text,
    )));

    // 2. Checkboxes (Muted/Success, 2-space indent)
    form.add_widget(Box::new(CheckboxWidget::new(
        "enable_tls",
        "Enable Automatic TLS / SSL Certificate",
        true,
    )));
    form.add_widget(Box::new(CheckboxWidget::new(
        "telemetry",
        "Send Anonymous Telemetry & Crash Reports",
        false,
    )));

    // 3. Radio Options (Accent for selected, Muted for unselected)
    form.add_widget(Box::new(RadioWidget::new(
        "environment",
        "Deployment Target",
        vec![
            "Production".into(),
            "Preview".into(),
            "Development".into(),
        ],
    )));

    // 4. TaskWidget với Spinner Thinking / Pulse
    let deploy_task = Arc::new(Mutex::new(
        TaskWidget::new_loading(
            "deploy_agent",
            "Agent Verification",
            "Connecting to edge cluster...",
            Theme::ACCENT,
        )
        .with_spinner_type(SpinnerType::Pulse),
    ));
    form.add_widget(Box::new(TaskShared(Arc::clone(&deploy_task))));

    // 5. Buttons (▸ prefix, Reverse video on focused)
    form.add_widget(Box::new(
        ButtonWidget::new("btn_deploy", "Deploy Now", Theme::BG, Theme::FG),
    ));
    form.add_widget(Box::new(
        ButtonWidget::new("btn_cancel", "Cancel", Theme::BG, Theme::MUTED),
    ));

    let tick_interval = Duration::from_millis(150); // Nhịp Pulse 150ms
    let mut last_tick = Instant::now();
    let mut needs_render = true;

    loop {
        let elapsed = last_tick.elapsed();
        let timeout = if elapsed >= tick_interval {
            Duration::from_millis(0)
        } else {
            tick_interval - elapsed
        };

        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    let is_ctrl_c = key.modifiers.contains(KeyModifiers::CONTROL)
                        && key.code == KeyCode::Char('c');
                    if key.code == KeyCode::Esc || is_ctrl_c {
                        break;
                    }

                    let res = form.handle_event(key);
                    needs_render = true;
                    if res == EventResult::Submitted {
                        break;
                    }
                }
            }
        }

        if last_tick.elapsed() >= tick_interval {
            if let Ok(mut w) = deploy_task.lock() {
                w.tick();
            }
            last_tick = Instant::now();
            needs_render = true;
        }

        if needs_render {
            terminal.draw(|f| {
                let full_area = f.area();

                // Bố cục Layout: Top Header (H1), Main Container (Panel), Bottom Status Bar
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Length(3), // Header title
                        Constraint::Min(12),   // Main panel
                        Constraint::Length(1), // Status bar
                    ])
                    .split(full_area);

                // Header H1: Minimal, BOLD + Primary
                let header = Paragraph::new(Line::from(vec![
                    Span::styled("  LINEAR / VERCEL TUI", Style::default().fg(Theme::PRIMARY).add_modifier(Modifier::BOLD)),
                    Span::styled(" ─ Service Configuration", Style::default().fg(Theme::SECONDARY)),
                ]))
                .style(Style::default().bg(Theme::BG));
                f.render_widget(header, chunks[0]);

                // Main Panel (Panel / Card với Single-line Box Drawing và Embedded Title)
                let panel_block = Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Plain)
                    .border_style(Style::default().fg(Theme::NEUTRAL_100))
                    .title(Span::styled("─ Deployment Settings ─", Style::default().fg(Theme::SECONDARY)))
                    .style(Style::default().bg(Theme::BG));

                let panel_inner = panel_block.inner(chunks[1]);
                f.render_widget(panel_block, chunks[1]);

                // Render toàn bộ Form bên trong panel
                form.render(panel_inner, f);

                // Status Bar (Single line at bottom, separated by " ─ ")
                let status_line = Line::from(vec![
                    Span::styled(" main ", Style::default().fg(Theme::PRIMARY).add_modifier(Modifier::BOLD)),
                    Span::styled("─", Style::default().fg(Theme::MUTED)),
                    Span::styled(" Tab: Next ", Style::default().fg(Theme::SECONDARY)),
                    Span::styled("─", Style::default().fg(Theme::MUTED)),
                    Span::styled(" ↑/↓: Navigate ", Style::default().fg(Theme::SECONDARY)),
                    Span::styled("─", Style::default().fg(Theme::MUTED)),
                    Span::styled(" Enter/Space: Select ", Style::default().fg(Theme::SECONDARY)),
                    Span::styled("─", Style::default().fg(Theme::MUTED)),
                    Span::styled(" Esc: Exit ", Style::default().fg(Theme::SECONDARY)),
                    Span::styled("                              ✓ System Ready ", Style::default().fg(Theme::SUCCESS)),
                ]);
                let status_bar = Paragraph::new(status_line).style(Style::default().bg(Theme::BG));
                f.render_widget(status_bar, chunks[2]);
            })?;
            needs_render = false;
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    println!("\n=== FORM SUBMITTED / FINISHED ===");
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
