// --- PHÂN ĐOẠN: DEMO KIỂM THỬ TRỰC QUAN TOÀN BỘ ICONS & COLOR PALETTE (DESIGN.MD) ---

use cli_gui_form::{Icons, Theme};
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
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

    let mut think_idx = 0;
    let mut braille_idx = 0;
    let mut last_tick_150ms = Instant::now();
    let mut last_tick_80ms = Instant::now();
    let mut needs_render = true;

    const BRAILLE_SPINNER: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

    loop {
        // Nhịp quay Braille Spinner chuẩn 80ms theo DESIGN.md Mục 8
        if last_tick_80ms.elapsed() >= Duration::from_millis(80) {
            braille_idx = (braille_idx + 1) % BRAILLE_SPINNER.len();
            last_tick_80ms = Instant::now();
            needs_render = true;
        }

        // Nhịp nháy Thinking Pulse chuẩn 150ms theo DESIGN.md Mục 8
        if last_tick_150ms.elapsed() >= Duration::from_millis(150) {
            think_idx = (think_idx + 1) % Icons::THINKING_FRAMES.len();
            last_tick_150ms = Instant::now();
            needs_render = true;
        }

        if needs_render {
            terminal.draw(|f| {
                let full_area = f.area();

                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Length(3), // Header
                        Constraint::Min(14),   // Content linh hoạt theo mọi chiều cao terminal
                        Constraint::Length(1), // Footer
                    ])
                    .split(full_area);

                // 1. Header
                let header = Paragraph::new(Line::from(vec![
                    Span::styled("  LINEAR / VERCEL TUI", Style::default().fg(Theme::PRIMARY).add_modifier(Modifier::BOLD)),
                    Span::styled(" ─ Palette, Icons & Motion Showcase", Style::default().fg(Theme::SECONDARY)),
                ]))
                .style(Style::default().bg(Theme::BG));
                f.render_widget(header, chunks[0]);

                // 2. Chia đôi màn hình: Cột trái (Color Palette & Neutral Scale), Cột phải (Icons, Spinner & Progress)
                let main_cols = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([
                        Constraint::Percentage(50),
                        Constraint::Percentage(50),
                    ])
                    .split(chunks[1]);

                // PANEL TRÁI: Color Palette & Neutral Scale
                let color_block = Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(Theme::NEUTRAL_100))
                    .title(Line::from(vec![
                        Span::styled("─ ", Style::default().fg(Theme::NEUTRAL_100)),
                        Span::styled("Color Palette & Neutral Scale", Style::default().fg(Theme::PRIMARY).add_modifier(Modifier::BOLD)),
                        Span::styled(" ─", Style::default().fg(Theme::NEUTRAL_100)),
                    ]))
                    .style(Style::default().bg(Theme::BG));

                let color_lines = vec![
                    Line::from(Span::styled("  Semantic Roles (Màu theo ngữ nghĩa):", Style::default().fg(Theme::SECONDARY).add_modifier(Modifier::BOLD))),
                    Line::from(vec![
                        Span::styled("   ■ ", Style::default().fg(Theme::BG)),
                        Span::styled("Background  ", Style::default().fg(Theme::FG)),
                        Span::styled("#0a0a0a  (Main canvas)", Style::default().fg(Theme::MUTED)),
                    ]),
                    Line::from(vec![
                        Span::styled("   ■ ", Style::default().fg(Theme::FG)),
                        Span::styled("Foreground  ", Style::default().fg(Theme::FG)),
                        Span::styled("#ededed  (Default text)", Style::default().fg(Theme::MUTED)),
                    ]),
                    Line::from(vec![
                        Span::styled("   ■ ", Style::default().fg(Theme::PRIMARY)),
                        Span::styled("Primary     ", Style::default().fg(Theme::PRIMARY).add_modifier(Modifier::BOLD)),
                        Span::styled("#ffffff  (Key actions, focus)", Style::default().fg(Theme::MUTED)),
                    ]),
                    Line::from(vec![
                        Span::styled("   ■ ", Style::default().fg(Theme::SECONDARY)),
                        Span::styled("Secondary   ", Style::default().fg(Theme::SECONDARY)),
                        Span::styled("#888888  (Supporting text)", Style::default().fg(Theme::MUTED)),
                    ]),
                    Line::from(vec![
                        Span::styled("   ■ ", Style::default().fg(Theme::ACCENT)),
                        Span::styled("Accent      ", Style::default().fg(Theme::ACCENT).add_modifier(Modifier::BOLD)),
                        Span::styled("#0070f3  (Vercel Blue links)", Style::default().fg(Theme::MUTED)),
                    ]),
                    Line::from(vec![
                        Span::styled("   ■ ", Style::default().fg(Theme::SUCCESS)),
                        Span::styled("Success     ", Style::default().fg(Theme::SUCCESS)),
                        Span::styled("#00c853  (Positive status)", Style::default().fg(Theme::MUTED)),
                    ]),
                    Line::from(vec![
                        Span::styled("   ■ ", Style::default().fg(Theme::WARNING)),
                        Span::styled("Warning     ", Style::default().fg(Theme::WARNING)),
                        Span::styled("#f5a623  (Caution status)", Style::default().fg(Theme::MUTED)),
                    ]),
                    Line::from(vec![
                        Span::styled("   ■ ", Style::default().fg(Theme::ERROR)),
                        Span::styled("Error       ", Style::default().fg(Theme::ERROR)),
                        Span::styled("#ee0000  (Error status)", Style::default().fg(Theme::MUTED)),
                    ]),
                    Line::from(vec![
                        Span::styled("   ■ ", Style::default().fg(Theme::MUTED)),
                        Span::styled("Muted       ", Style::default().fg(Theme::MUTED)),
                        Span::styled("#555555  (Disabled, hints)", Style::default().fg(Theme::MUTED)),
                    ]),
                    Line::from(vec![
                        Span::styled("   ■ ", Style::default().fg(Theme::SURFACE)),
                        Span::styled("Surface     ", Style::default().fg(Theme::SURFACE)),
                        Span::styled("#1a1a1a  (Panels, cards)", Style::default().fg(Theme::MUTED)),
                    ]),
                    Line::from(""),
                    Line::from(Span::styled("  Neutral Scale (Thang xám phân cấp độ sâu):", Style::default().fg(Theme::SECONDARY).add_modifier(Modifier::BOLD))),
                    Line::from(vec![
                        Span::styled("   ■ ", Style::default().fg(Theme::NEUTRAL_50)),
                        Span::styled("Step 50  (#1a1a1a) -> Subtle bg, surface", Style::default().fg(Theme::MUTED)),
                    ]),
                    Line::from(vec![
                        Span::styled("   ■ ", Style::default().fg(Theme::NEUTRAL_100)),
                        Span::styled("Step 100 (#2a2a2a) -> Borders, dividers", Style::default().fg(Theme::MUTED)),
                    ]),
                    Line::from(vec![
                        Span::styled("   ■ ", Style::default().fg(Theme::NEUTRAL_200)),
                        Span::styled("Step 200 (#444444) -> Disabled text", Style::default().fg(Theme::MUTED)),
                    ]),
                    Line::from(vec![
                        Span::styled("   ■ ", Style::default().fg(Theme::NEUTRAL_300)),
                        Span::styled("Step 300 (#666666) -> Placeholder text", Style::default().fg(Theme::MUTED)),
                    ]),
                    Line::from(vec![
                        Span::styled("   ■ ", Style::default().fg(Theme::NEUTRAL_400)),
                        Span::styled("Step 400 (#888888) -> Secondary text", Style::default().fg(Theme::MUTED)),
                    ]),
                    Line::from(vec![
                        Span::styled("   ■ ", Style::default().fg(Theme::NEUTRAL_500)),
                        Span::styled("Step 500 (#ededed) -> Body text", Style::default().fg(Theme::MUTED)),
                    ]),
                ];
                let p_left = Paragraph::new(color_lines).block(color_block);
                f.render_widget(p_left, main_cols[0]);

                // PANEL PHẢI: Icons, Spinners & Progress Bar
                let icon_block = Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(Theme::NEUTRAL_100))
                    .title(Line::from(vec![
                        Span::styled("─ ", Style::default().fg(Theme::NEUTRAL_100)),
                        Span::styled("Icons, Spinners & Progress (Mục 7 & 8)", Style::default().fg(Theme::PRIMARY).add_modifier(Modifier::BOLD)),
                        Span::styled(" ─", Style::default().fg(Theme::NEUTRAL_100)),
                    ]))
                    .style(Style::default().bg(Theme::BG));

                let cur_think = Icons::THINKING_FRAMES[think_idx];
                let cur_braille = BRAILLE_SPINNER[braille_idx];

                // Căn chuẩn cột không bị lệch do khoảng trắng kép của các icon rộng
                let icon_lines = vec![
                    Line::from(Span::styled("  Status Indicators (Mỗi icon chiếm chuẩn 2 ô):", Style::default().fg(Theme::SECONDARY).add_modifier(Modifier::BOLD))),
                    Line::from(vec![
                        Span::raw("   "),
                        Span::styled(Icons::SUCCESS, Style::default().fg(Icons::color_success())),
                        Span::styled("Success           ", Style::default().fg(Theme::FG)),
                        Span::styled(Icons::ERROR, Style::default().fg(Icons::color_error())),
                        Span::styled("Error", Style::default().fg(Theme::FG)),
                    ]),
                    Line::from(vec![
                        Span::raw("   "),
                        Span::styled(Icons::WARNING, Style::default().fg(Icons::color_warning())),
                        Span::styled("Warning           ", Style::default().fg(Theme::FG)),
                        Span::styled(Icons::INFO, Style::default().fg(Icons::color_info())),
                        Span::styled("Info", Style::default().fg(Theme::FG)),
                    ]),
                    Line::from(vec![
                        Span::raw("   "),
                        Span::styled(Icons::RUN, Style::default().fg(Icons::color_run())),
                        Span::styled("Running           ", Style::default().fg(Theme::FG)),
                        Span::styled(Icons::BUILD, Style::default().fg(Icons::color_build())),
                        Span::styled("Build", Style::default().fg(Theme::FG)),
                    ]),
                    Line::from(vec![
                        Span::raw("   "),
                        Span::styled(Icons::STOP, Style::default().fg(Icons::color_stop())),
                        Span::styled("Stop              ", Style::default().fg(Theme::FG)),
                        Span::styled(Icons::PAUSE, Style::default().fg(Icons::color_pending())),
                        Span::styled("Pause", Style::default().fg(Theme::FG)),
                    ]),
                    Line::from(""),
                    Line::from(Span::styled("  Controls & Selectors:", Style::default().fg(Theme::SECONDARY).add_modifier(Modifier::BOLD))),
                    Line::from(vec![
                        Span::raw("   "),
                        Span::styled(Icons::CHECKBOX_ON, Style::default().fg(Theme::SUCCESS)),
                        Span::styled("Checkbox On       ", Style::default().fg(Theme::FG)),
                        Span::styled(Icons::CHECKBOX_OFF, Style::default().fg(Theme::MUTED)),
                        Span::styled("Checkbox Off", Style::default().fg(Theme::FG)),
                    ]),
                    Line::from(vec![
                        Span::raw("   "),
                        Span::styled(Icons::RADIO_ON, Style::default().fg(Theme::ACCENT)),
                        Span::styled("Radio On          ", Style::default().fg(Theme::FG)),
                        Span::styled(Icons::RADIO_OFF, Style::default().fg(Theme::MUTED)),
                        Span::styled("Radio Off", Style::default().fg(Theme::FG)),
                    ]),
                    Line::from(vec![
                        Span::raw("   "),
                        Span::styled(Icons::POINTER, Style::default().fg(Theme::PRIMARY).add_modifier(Modifier::BOLD)),
                        Span::styled("Selected          ", Style::default().fg(Theme::FG)),
                        Span::styled(Icons::ARROW_RIGHT, Style::default().fg(Theme::SECONDARY)),
                        Span::styled("Arrow", Style::default().fg(Theme::FG)),
                    ]),
                    Line::from(vec![
                        Span::raw("   "),
                        Span::styled(Icons::BRANCH, Style::default().fg(Theme::ACCENT)),
                        Span::styled("Branch            ", Style::default().fg(Theme::FG)),
                        Span::styled(Icons::BULLET, Style::default().fg(Theme::SECONDARY)),
                        Span::styled("Bullet", Style::default().fg(Theme::FG)),
                    ]),
                    Line::from(vec![
                        Span::raw("   "),
                        Span::styled(Icons::SPARKLE_FILLED, Style::default().fg(Theme::WARNING)),
                        Span::styled("Sparkle           ", Style::default().fg(Theme::FG)),
                        Span::styled(Icons::SPARKLE_EMPTY, Style::default().fg(Theme::MUTED)),
                        Span::styled("Sparkle Empty", Style::default().fg(Theme::FG)),
                    ]),
                    Line::from(vec![
                        Span::raw("   "),
                        Span::styled(Icons::STAR_OUTLINE, Style::default().fg(Theme::PRIMARY)),
                        Span::styled("Star Outline      ", Style::default().fg(Theme::FG)),
                        Span::styled(Icons::DIAMOND_EMPTY, Style::default().fg(Theme::SECONDARY)),
                        Span::styled("Diamond", Style::default().fg(Theme::FG)),
                    ]),
                    Line::from(vec![
                        Span::raw("   "),
                        Span::styled(Icons::SNOWFLAKE, Style::default().fg(Theme::ACCENT)),
                        Span::styled("Snowflake         ", Style::default().fg(Theme::FG)),
                        Span::styled(Icons::ARROW_UP, Style::default().fg(Theme::PRIMARY)),
                        Span::styled("Up Triangle", Style::default().fg(Theme::FG)),
                    ]),
                    Line::from(""),
                    Line::from(Span::styled("  Motion & Spinners (Mục 8):", Style::default().fg(Theme::SECONDARY).add_modifier(Modifier::BOLD))),
                    Line::from(vec![
                        Span::styled(format!("   {} ", cur_braille), Style::default().fg(Theme::ACCENT).add_modifier(Modifier::BOLD)),
                        Span::styled("Braille Spinner (80ms)", Style::default().fg(Theme::FG)),
                    ]),
                    Line::from(vec![
                        Span::styled(format!("   {} ", cur_think), Style::default().fg(Theme::PRIMARY).add_modifier(Modifier::BOLD)),
                        Span::styled("Thinking Pulse (150ms)", Style::default().fg(Theme::FG)),
                    ]),
                    Line::from(""),
                    Line::from(Span::styled("  Progress Bar (DESIGN.md Mục 8):", Style::default().fg(Theme::SECONDARY).add_modifier(Modifier::BOLD))),
                    Line::from(vec![
                        Span::styled("   ▕", Style::default().fg(Theme::NEUTRAL_100)), // Left Cap ▕
                        Span::styled("━━━━━━━━━━", Style::default().fg(Theme::ACCENT).add_modifier(Modifier::BOLD)), // Filled ━
                        Span::styled("──────────", Style::default().fg(Theme::MUTED)), // Empty ─
                        Span::styled("▏", Style::default().fg(Theme::NEUTRAL_100)), // Right Cap ▏
                        Span::styled(" 50%", Style::default().fg(Theme::PRIMARY).add_modifier(Modifier::BOLD)),
                    ]),
                ];
                let paragraph = Paragraph::new(icon_lines).block(icon_block);
                f.render_widget(paragraph, main_cols[1]);

                // 3. Status Bar (Tự động co giãn theo chiều rộng terminal)
                let width_avail = chunks[2].width.saturating_sub(2) as usize;
                let left_txt = " main ─ Press Esc to Exit ";
                let right_txt = "✓ 100% Design Matched ";
                let space_count = width_avail.saturating_sub(left_txt.len() + right_txt.len());
                let spaces = " ".repeat(space_count);

                let status_line = Line::from(vec![
                    Span::styled(" main ", Style::default().fg(Theme::PRIMARY).add_modifier(Modifier::BOLD)),
                    Span::styled("─", Style::default().fg(Theme::MUTED)),
                    Span::styled(" Press Esc to Exit ", Style::default().fg(Theme::SECONDARY)),
                    Span::raw(spaces),
                    Span::styled("✓ 100% Design Matched ", Style::default().fg(Theme::SUCCESS)),
                ]);
                let status_bar = Paragraph::new(status_line).style(Style::default().bg(Theme::BG));
                f.render_widget(status_bar, chunks[2]);
            })?;
            needs_render = false;
        }

        if event::poll(Duration::from_millis(80))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    let is_ctrl_c = key.modifiers.contains(KeyModifiers::CONTROL)
                        && key.code == KeyCode::Char('c');
                    if key.code == KeyCode::Esc || is_ctrl_c {
                        break;
                    }
                }
            }
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    println!("\n=== SHOWCASE HOÀN TẤT ===");
    Ok(())
}
