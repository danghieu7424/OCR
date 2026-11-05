// --- PHÂN ĐOẠN: DEMO TABS & STATUS BAR CHUẨN MINIMAL TUI DESIGN SYSTEM ---

use cli_gui_form::{FormWidget, StatusBarWidget, TabsWidget, Theme};
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Modifier, Style},
    text::{Line, Span},
    Terminal,
};
use std::{
    io::{self, stdout},
    time::{Duration, Instant},
};

fn main() -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut tabs = TabsWidget::new("main_nav", vec!["Overview", "Logs", "Settings", "Deployments"]);
    tabs.focus();

    // 1. Khởi tạo danh sách log động mô phỏng server streaming
    let mut streaming_logs: Vec<Line<'static>> = vec![
        Line::from(vec![
            Span::styled("14:22:01 ", Style::default().fg(Theme::MUTED)),
            Span::styled("[INFO] ", Style::default().fg(Theme::ACCENT)),
            Span::styled("HTTP server listening at 127.0.0.1:3000", Style::default().fg(Theme::FG)),
        ]),
        Line::from(vec![
            Span::styled("14:22:05 ", Style::default().fg(Theme::MUTED)),
            Span::styled("[OK]   ", Style::default().fg(Theme::SUCCESS)),
            Span::styled("Database migration applied (version 0.4.2)", Style::default().fg(Theme::FG)),
        ]),
        Line::from(vec![
            Span::styled("14:22:18 ", Style::default().fg(Theme::MUTED)),
            Span::styled("[BUILD]", Style::default().fg(Theme::PRIMARY)),
            Span::styled("Compiled client assets in 480ms", Style::default().fg(Theme::FG)),
        ]),
        Line::from(vec![
            Span::styled("14:22:25 ", Style::default().fg(Theme::MUTED)),
            Span::styled("[DEBUG]", Style::default().fg(Theme::SECONDARY)),
            Span::styled("Long payload stream: {\"user_id\":1092837,\"token\":\"eyJhbGciOiJIUzI1Ni...\"}", Style::default().fg(Theme::MUTED)),
        ]),
    ];

    let mut last_log_time = Instant::now();
    let mut log_counter = 1;

    loop {
        // Sinh log mới mỗi 1.8 giây để kiểm thử cơ chế Sticky Follow
        if last_log_time.elapsed() >= Duration::from_millis(1800) {
            let now_sec = 25 + log_counter * 2;
            let time_str = format!("14:22:{:02}", now_sec % 60);
            let (level, color, msg) = match log_counter % 5 {
                0 => ("[OK]   ", Theme::SUCCESS, format!("Batch task #{} completed successfully", log_counter)),
                1 => ("[INFO] ", Theme::ACCENT, format!("Worker dispatched payload #{} to background pool", log_counter)),
                2 => ("[DEBUG]", Theme::SECONDARY, format!("Cache hit on query key 'user_session_{}' (0.3ms)", log_counter)),
                3 => ("[WARN] ", Theme::WARNING, format!("High CPU spike detected on worker core #{} (82%)", (log_counter % 4) + 1)),
                _ => ("[INFO] ", Theme::PRIMARY, format!("Heartbeat probe received from edge gateway #{}", log_counter)),
            };

            streaming_logs.push(Line::from(vec![
                Span::styled(format!("{} ", time_str), Style::default().fg(Theme::MUTED)),
                Span::styled(format!("{} ", level), Style::default().fg(color)),
                Span::styled(msg, Style::default().fg(Theme::FG)),
            ]));

            log_counter += 1;
            last_log_time = Instant::now();
        }

        terminal.draw(|f| {
            let size = f.area();

            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Min(10),   // 1. Tabbed Panel Container (Nối liền toàn bộ Tab + Panel bo góc)
                    Constraint::Length(1), // 2. Status Bar
                ])
                .split(size);

            // 1. Nội dung theo Tab đang chọn
            let tab_content = match tabs.selected() {
                0 => vec![
                    Line::from(vec![
                        Span::styled("● ", Style::default().fg(Theme::SUCCESS)),
                        Span::styled("System Status: ", Style::default().fg(Theme::SECONDARY)),
                        Span::styled("All services operational", Style::default().fg(Theme::PRIMARY).add_modifier(Modifier::BOLD)),
                    ]),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled("  Uptime:       ", Style::default().fg(Theme::SECONDARY)),
                        Span::styled("99.98% (34 days, 12 hours)", Style::default().fg(Theme::FG)),
                    ]),
                    Line::from(vec![
                        Span::styled("  Active Nodes: ", Style::default().fg(Theme::SECONDARY)),
                        Span::styled("12 / 12 clusters ready", Style::default().fg(Theme::FG)),
                    ]),
                ],
                1 => streaming_logs.clone(),
                2 => vec![
                    Line::from(vec![
                        Span::styled("  Theme Mode:     ", Style::default().fg(Theme::SECONDARY)),
                        Span::styled("Minimal Dark (256-color / TrueColor)", Style::default().fg(Theme::FG)),
                    ]),
                    Line::from(vec![
                        Span::styled("  Refresh Rate:   ", Style::default().fg(Theme::SECONDARY)),
                        Span::styled("60 FPS (16ms frame interval)", Style::default().fg(Theme::FG)),
                    ]),
                    Line::from(vec![
                        Span::styled("  Log Verbosity:  ", Style::default().fg(Theme::SECONDARY)),
                        Span::styled("Verbose / Debug", Style::default().fg(Theme::ACCENT)),
                    ]),
                ],
                _ => vec![
                    Line::from(vec![
                        Span::styled("  Branch: ", Style::default().fg(Theme::SECONDARY)),
                        Span::styled("main (commit 1c905d4)", Style::default().fg(Theme::PRIMARY).add_modifier(Modifier::BOLD)),
                    ]),
                    Line::from(vec![
                        Span::styled("  Latest Check: ", Style::default().fg(Theme::SECONDARY)),
                        Span::styled("✔ 12/12 tests passed", Style::default().fg(Theme::SUCCESS)),
                    ]),
                ],
            };

            // 2. Render Tab Container nối liền bo góc hoàn chỉnh
            tabs.render_container(chunks[0], tab_content, f);

            // 3. Status Bar Widget với Sticky Follow indicator
            let mut status_bar = StatusBarWidget::new();
            status_bar.add_left(Span::styled("Tab: [Left/Right/Tab]", Style::default().fg(Theme::PRIMARY)));
            status_bar.add_left(Span::styled("Scroll: [Up/Down/PgUp/PgDn]", Style::default().fg(Theme::SECONDARY)));
            
            if tabs.is_auto_scroll() {
                status_bar.add_left(Span::styled("Sticky: [ON - Follow Latest]", Style::default().fg(Theme::SUCCESS).add_modifier(Modifier::BOLD)));
            } else {
                status_bar.add_left(Span::styled("Sticky: [PAUSED - Press End to Follow]", Style::default().fg(Theme::WARNING)));
            }

            status_bar.add_right(Span::styled(format!("Logs: {}", streaming_logs.len()), Style::default().fg(Theme::ACCENT)));
            status_bar.add_right(Span::styled("Press 'q' to exit", Style::default().fg(Theme::MUTED)));

            status_bar.render(chunks[1], f);
        })?;

        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => break,
                        _ => {
                            tabs.handle_event(key);
                        }
                    }
                }
            }
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    println!("=== DEMO TABS & STATUS BAR HOÀN TẤT ===");
    Ok(())
}
