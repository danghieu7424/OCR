// --- TEST ĐỘC LẬP: TEST HIỂN THỊ NÚT BẤM PODCAST VÀ KIỂM TRA CO RÚT ---
// File: examples/test_podcast_button.rs
// Mục đích: Cho phép người dùng test trực tiếp nút "▶ BẮT ĐẦU TẠO PODCAST (Enter)"
//          để kiểm tra xem glyph Unicode '▶' và tiếng Việt có bị co, méo, hay giật lề khi Focus/Blur.

use cli_gui_form::{
    ButtonWidget, EventResult, FormManager, Icons,
    InputMode, InputWidget, SelectWidget, Theme,
};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
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
    io::stdout,
    time::Duration,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // 1. Khởi tạo Form Manager chứa form mẫu Podcast
    let mut form = FormManager::new();

    form.add_widget(Box::new(
        InputWidget::new("podcast_title", "Tiêu đề tập Podcast", InputMode::Text)
            .with_placeholder("Ví dụ: Hành trình khám phá AI Agents 2026..."),
    ));

    form.add_widget(Box::new(
        SelectWidget::new(
            "voice_model",
            "Mô hình giọng đọc (Voice TTS)",
            vec![
                ("gemini-flash", "Gemini 2.5 Flash Audio (Neural High-Def)"),
                ("elevenlabs", "ElevenLabs Multi-Lingual v2 (Studio Voice)"),
                ("openai-tts", "OpenAI TTS-1 HD (Onyx / Nova)"),
            ],
        )
        .with_selected(0),
    ));

    // Nút 1: Kiểu Invert High-Contrast (Linear / Apple Dark Mode - Nền đen chữ xám, hover nền trắng chữ đen)
    form.add_widget(Box::new(
        ButtonWidget::new(
            "btn_invert",
            "BẮT ĐẦU TẠO PODCAST (Enter) ─ [Invert High-Contrast]",
            Theme::BG,
            Theme::SECONDARY,
        )
        .with_icon(Icons::RUN)
        .with_preset(Theme::BTN_INVERT),
    ));

    // Nút 2: Kiểu Solid Accent CTA (Vercel Blue)
    form.add_widget(Box::new(
        ButtonWidget::new(
            "btn_accent",
            "BẮT ĐẦU TẠO PODCAST (Enter) ─ [Solid Vercel Blue]",
            Theme::ACCENT,
            Theme::WHITE,
        )
        .with_icon(Icons::RUN)
        .with_full_width(true)
        .with_centered(true)
        .with_bordered(true)
        .with_preset(Theme::BTN_ACCENT),
    ));

    // Nút 3: Kiểu AI Intelligence (Linear Violet - Cực hợp với AI Dubbing/Podcast)
    form.add_widget(Box::new(
        ButtonWidget::new(
            "btn_purple",
            "BẮT ĐẦU TẠO PODCAST (Enter) ─ [AI Violet / Neural]",
            Theme::PURPLE,
            Theme::WHITE,
        )
        .with_icon(Icons::RUN)
        .with_full_width(true)
        .with_preset(Theme::BTN_PURPLE),
    ));

    // Nút 4: Kiểu Emerald Success (Green Production)
    form.add_widget(Box::new(
        ButtonWidget::new(
            "btn_emerald",
            "BẮT ĐẦU TẠO PODCAST (Enter) ─ [Emerald Success]",
            Theme::BG,
            Theme::EMERALD,
        )
        .with_icon(Icons::RUN)
        .with_preset(Theme::BTN_EMERALD),
    ));

    // Nút 5: Kiểu Minimal Ghost (GitHub CLI Style - Nhẹ nhàng, thanh lịch)
    form.add_widget(Box::new(
        ButtonWidget::new(
            "btn_minimal",
            "BẮT ĐẦU TẠO PODCAST (Enter) ─ [Minimal Ghost]",
            Theme::BG,
            Theme::SECONDARY,
        )
        .with_icon(Icons::RUN)
        .with_preset(Theme::BTN_GHOST),
    ));

    // Nút 6: [THEO YÊU CẦU BẠN] Bật BOLD cho Text nhưng Icon ▶ tuyệt đối KHÔNG BOLD (trên nền Solid Accent an toàn)
    form.add_widget(Box::new(
        ButtonWidget::new(
            "btn_bold_text_only",
            "BẮT ĐẦU TẠO PODCAST (Enter) ─ [Text BOLD / Icon Regular]",
            Theme::ACCENT,
            Theme::WHITE,
        )
        .with_icon(Icons::RUN)
        .with_full_width(true)
        .with_bold(true) // Kích hoạt BOLD cho text, Icon ▶ tự động được bóc tách và gỡ cờ BOLD
        .with_preset(Theme::BTN_ACCENT),
    ));

    let mut last_action = String::from("Sẵn sàng. Dùng Tab / ↑ / ↓ để di chuyển focus vào các nút.");

    loop {
        terminal.draw(|f| {
            let size = f.area();

            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3), // Header
                    Constraint::Min(16),   // Form & Buttons
                    Constraint::Length(4), // Footer & Status
                ])
                .split(size);

            // 1. Header
            let header = Paragraph::new(Line::from(vec![
                Span::styled(format!("  {} TEST CHỐNG CO RÚT NÚT BẤM (BUTTON INSPECTOR)", Icons::RUN), Style::default().fg(Theme::PRIMARY).add_modifier(Modifier::BOLD)),
                Span::styled(" ─ Kiểm tra độ ổn định Unicode & Font", Style::default().fg(Theme::SECONDARY)),
            ]))
            .block(Block::default().borders(Borders::BOTTOM).border_style(Style::default().fg(Theme::NEUTRAL_100)));
            f.render_widget(header, chunks[0]);

            // 2. Body Panel
            let panel_block = Block::default()
                .title("─ Podcast Generation Form ─")
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Theme::BORDER_FOCUS))
                .style(Style::default().bg(Theme::BG));

            let inner_area = Rect {
                x: chunks[1].x + 2,
                y: chunks[1].y + 1,
                width: chunks[1].width.saturating_sub(4),
                height: chunks[1].height.saturating_sub(2),
            };

            f.render_widget(panel_block, chunks[1]);
            form.render(inner_area, f);

            // 3. Footer status
            let footer = Paragraph::new(vec![
                Line::from(vec![
                    Span::styled("  Trạng thái: ", Style::default().fg(Theme::MUTED)),
                    Span::styled(&last_action, Style::default().fg(Theme::SUCCESS).add_modifier(Modifier::BOLD)),
                ]),
                Line::from(vec![
                    Span::styled("  Điều khiển: [Tab / ↑ / ↓] Chuyển nút  ─  [Enter] Kích hoạt nút  ─  [Esc] Thoát", Style::default().fg(Theme::MUTED)),
                ]),
            ])
            .block(Block::default().borders(Borders::TOP).border_style(Style::default().fg(Theme::NEUTRAL_100)));
            f.render_widget(footer, chunks[2]);
        })?;

        if event::poll(Duration::from_millis(30))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    if key.code == KeyCode::Esc || key.code == KeyCode::Char('q') {
                        break;
                    }

                    // Form xử lý phím
                    let res = form.handle_event(key);
                    if res == EventResult::Submitted {
                        last_action = format!("✔ Đã nhấn kích hoạt: '▶ BẮT ĐẦU TẠO PODCAST (Enter)' thành công!");
                    }
                }
            }
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;
    println!("=== TEST PODCAST BUTTON HOÀN TẤT ===");
    Ok(())
}
