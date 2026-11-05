// --- PHÂN ĐOẠN: MASTER SHOWCASE TOÀN DIỆN DESIGN.MD VÀ MỌI WIDGET ---

use cli_gui_form::{
    ButtonWidget, CardWidget, CheckboxWidget, EditableListWidget, EventResult, FormManager, FormWidget, Icons,
    InputMode, InputWidget, ListWidget, ProgressStyle, RadioWidget, SelectWidget, ShimmerWidget, SpinnerType, StatusBarWidget, TabsWidget,
    Tags, TaskWidget, Theme,
};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Terminal,
};
use std::{
    io::{self, stdout},
    time::{Duration, Instant},
};

struct TableRowItem {
    name: &'static str,
    status_icon: &'static str,
    status_text: &'static str,
    status_color: Color,
    branch: &'static str,
    commit: &'static str,
    latency: &'static str,
    time: &'static str,
}

fn main() -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // 1. TABS WIDGET VỚI 5 PHÂN HỆ DESIGN.MD
    let mut tabs = TabsWidget::new(
        "master_tabs",
        vec![
            "1. Forms & Inputs",
            "2. Table & Badges",
            "3. Tasks & Spinners",
            "4. Live Logs",
            "5. Icons & Theme",
        ],
    );
    tabs.focus();
    tabs.set_auto_scroll(false);

    // 2. KHỞI TẠO FORM SUITE (TAB 1)
    let mut form = FormManager::new();
    form.add_widget(Box::new(
        InputWidget::new("service_name", "Service Name", InputMode::Text)
            .with_placeholder("e.g. acme-auth-gateway"),
    ));
    form.add_widget(Box::new(
        InputWidget::new("api_secret", "API Secret Key", InputMode::Password)
            .with_placeholder("Enter secret key or paste token..."),
    ));
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
    form.add_widget(Box::new(RadioWidget::new(
        "environment",
        "Deployment Target",
        vec![
            "Production".into(),
            "Staging".into(),
            "Preview".into(),
            "Development".into(),
        ],
    )));
    form.add_widget(Box::new(
        SelectWidget::new(
            "region",
            "Hosting Datacenter (HTML Select/Option ─ 16 Regions)",
            vec![
                ("us-east-1", "US East (N. Virginia)"),
                ("us-east-2", "US East (Ohio)"),
                ("us-west-1", "US West (N. California)"),
                ("us-west-2", "US West (Oregon)"),
                ("ca-central-1", "Canada (Central)"),
                ("eu-west-1", "Europe (Ireland)"),
                ("eu-central-1", "Europe (Frankfurt) ─ High-Performance Low-Latency Financial Cloud Cluster"),
                ("eu-west-2", "Europe (London)"),
                ("eu-south-1", "Europe (Milan)"),
                ("ap-northeast-1", "Asia Pacific (Tokyo)"),
                ("ap-northeast-2", "Asia Pacific (Seoul)"),
                ("ap-southeast-1", "Asia Pacific (Singapore)"),
                ("ap-southeast-2", "Asia Pacific (Sydney)"),
                ("ap-south-1", "Asia Pacific (Mumbai)"),
                ("me-south-1", "Middle East (Bahrain)"),
                ("sa-east-1", "South America (São Paulo)"),
                ("us-gov-west-1", "AWS GovCloud (US-West) ─ Isolated High-Compliance Secure Enclave Multi-Zone Tier-4"),
            ],
        )
        .with_placeholder("Select deployment datacenter... ▾")
        .with_selected(0)
        .with_max_visible(6),
    ));
    form.add_widget(Box::new(
        EditableListWidget::new(
            "tags",
            "Tags / Environment Variables (CRUD List: Enter: Sửa/Thêm, Del: Xoá, Esc: Đóng)",
            vec!["v1.0-release", "cluster:eu-west", "monorepo-core"],
        )
        .with_max_visible(5)
        .with_max_items(8),
    ));
    form.add_widget(Box::new(
        ListWidget::new("route_menu")
            .with_label("Select Target Route (DESIGN.md Lists/Menus)")
            .with_item("api/routes.ts")
            .with_item("api/handler.ts")
            .with_disabled_item("lib/internal.ts (deprecated)")
            .with_item("config.json"),
    ));
    form.add_widget(Box::new(ButtonWidget::new(
        "btn_deploy",
        "Deploy Now",
        Theme::BG,
        Theme::FG,
    )));
    form.add_widget(Box::new(ButtonWidget::new(
        "btn_cancel",
        "Cancel",
        Theme::BG,
        Theme::MUTED,
    )));

    // 3. DỮ LIỆU TABLE (TAB 2)
    let table_items = vec![
        TableRowItem {
            name: "deploy-api",
            status_icon: Icons::SUCCESS,
            status_text: "Ready",
            status_color: Theme::SUCCESS,
            branch: "main",
            commit: "7670885",
            latency: "18ms",
            time: "2m ago",
        },
        TableRowItem {
            name: "deploy-web",
            status_icon: Icons::RUN,
            status_text: "Build",
            status_color: Theme::ORANGE,
            branch: "feat/tui",
            commit: "ffe5daf",
            latency: "45ms",
            time: "just now",
        },
        TableRowItem {
            name: "worker-ai",
            status_icon: Icons::WARNING,
            status_text: "High Load",
            status_color: Theme::WARNING,
            branch: "main",
            commit: "1a89c20",
            latency: "120ms",
            time: "8m ago",
        },
        TableRowItem {
            name: "deploy-docs",
            status_icon: Icons::ERROR,
            status_text: "Failed",
            status_color: Theme::CRITICAL,
            branch: "docs/v2",
            commit: "c381d09",
            latency: "---",
            time: "15m ago",
        },
        TableRowItem {
            name: "redis-cache",
            status_icon: Icons::PAUSE,
            status_text: "Paused",
            status_color: Theme::MUTED,
            branch: "infra",
            commit: "e29b11a",
            latency: "2ms",
            time: "1h ago",
        },
        TableRowItem {
            name: "cleanup-cron",
            status_icon: Icons::STOP,
            status_text: "Stopped",
            status_color: Theme::ERROR,
            branch: "infra",
            commit: "40fa882",
            latency: "---",
            time: "3h ago",
        },
    ];

    // Card 1: Plain border Deploy Status theo đúng chuẩn DESIGN.md mục 5
    let deploy_card = CardWidget::new("card_deploy", "Deploy Status")
        .with_item("Production", Some(Icons::SUCCESS), "Ready", Theme::SUCCESS)
        .with_item("Preview", Some(Icons::RUN), "Building", Theme::CYAN)
        .with_item("Staging", Some(Icons::SUCCESS), "Ready", Theme::SUCCESS);

    // Card 2: Rounded border Infrastructure Metrics (Màu sắc đa dạng sắc nét)
    let infra_card = CardWidget::new("card_infra", "Infrastructure")
        .with_rounded(true)
        .with_title_color(Theme::PURPLE)
        .with_item("PostgreSQL", Some(Icons::SUCCESS), "Healthy", Theme::EMERALD)
        .with_item("Redis Cluster", Some(Icons::WARNING), "Degraded", Theme::ORANGE)
        .with_item("Kafka Stream", Some(Icons::CIRCLE_TARGET), "Leader", Theme::CYAN)
        .with_item("Edge Router", Some(Icons::DIAMOND_FILLED), "Active", Theme::SKY);

    // 4. TASKS & ANIMATIONS (TAB 3)
    let mut pulse_task = TaskWidget::new_loading(
        "task_pulse",
        "Neural Inference Engine",
        "Synthesizing attention layers...",
        Theme::ACCENT,
    )
    .with_spinner_type(SpinnerType::Pulse);

    let mut braille_task = TaskWidget::new_loading(
        "task_braille",
        "Docker Layer Builder (6-Frame Braille)",
        "Exporting image snapshot sha256:7f8a9...",
        Theme::PRIMARY,
    )
    .with_spinner_type(SpinnerType::Braille6);

    let mut moon_task = TaskWidget::new_loading(
        "task_moon",
        "Cluster Orbit Synchronizer",
        "Rebalancing distributed partitions across nodes...",
        Theme::CYAN,
    )
    .with_spinner_type(SpinnerType::Moon);

    let mut arc_task = TaskWidget::new_loading(
        "task_arc",
        "Quadrant Arc Radar (4-Frame Arc)",
        "Scanning quadrant perimeter frequencies (U+25DC ─ U+25DF)...",
        Theme::EMERALD,
    )
    .with_spinner_type(SpinnerType::Arc);

    let progress_parallelogram = TaskWidget::new_progress(
        "task_progress_para",
        "Cyberpunk Parallelogram Bar",
        350,
        500,
        "blocks",
        "70%",
        "Syncing distributed ledger blocks (U+25B0 / U+25B1)...",
        Theme::CYAN,
    )
    .with_progress_style(ProgressStyle::Parallelogram);

    let progress_line = TaskWidget::new_progress(
        "task_progress_line",
        "Minimal Vercel Line Bar",
        480,
        500,
        "files",
        "96%",
        "Deploying edge functions to 24 datacenters (U+2501 / U+2500)...",
        Theme::EMERALD,
    )
    .with_progress_style(ProgressStyle::Line);

    let mut shimmer_bar = ShimmerWidget::new(
        "shimmer_bar",
        "Linear Sync Engine ─ Background Delta Syncing...",
    )
    .with_colors((69, 137, 255), (220, 240, 255));

    // 5. LIVE LOGS (TAB 4)
    let mut live_logs: Vec<Line<'static>> = vec![
        Line::from(vec![
            Span::styled("14:22:01 ", Style::default().fg(Theme::MUTED)),
            Span::styled(format!("{} ", Tags::INFO), Style::default().fg(Tags::color_for(Tags::INFO))),
            Span::styled("HTTP server listening at 127.0.0.1:3000", Style::default().fg(Theme::FG)),
        ]),
        Line::from(vec![
            Span::styled("14:22:05 ", Style::default().fg(Theme::MUTED)),
            Span::styled(format!("{} ", Tags::DONE), Style::default().fg(Tags::color_for(Tags::DONE))),
            Span::styled("Database schema migrations verified", Style::default().fg(Theme::FG)),
        ]),
        Line::from(vec![
            Span::styled("14:22:18 ", Style::default().fg(Theme::MUTED)),
            Span::styled(format!("{} ", Tags::EXEC), Style::default().fg(Tags::color_for(Tags::EXEC))),
            Span::styled("Native Rust backend compiled in 480ms", Style::default().fg(Theme::FG)),
        ]),
    ];

    let mut last_tick_60fps = Instant::now();
    let mut last_log_stream = Instant::now();
    let mut log_id = 1;

    loop {
        // Cập nhật nhịp animation 60 FPS
        if last_tick_60fps.elapsed() >= Duration::from_millis(16) {
            pulse_task.tick();
            braille_task.tick();
            moon_task.tick();
            arc_task.tick();
            shimmer_bar.tick();
            last_tick_60fps = Instant::now();
        }

        // Mô phỏng sinh log liên tục mỗi 1.6 giây
        if last_log_stream.elapsed() >= Duration::from_millis(1600) {
            let sec = 25 + log_id * 2;
            let time_str = format!("14:23:{:02}", sec % 60);
            let (tag, msg) = match log_id % 5 {
                0 => (Tags::DONE, format!("Event stream batch #{} acknowledged", log_id)),
                1 => (Tags::INFO, format!("Client session #{} authenticated via JWT", log_id)),
                2 => (Tags::CONF, format!("Redis cache hit for key 'cluster_metrics_{}'", log_id)),
                3 => (Tags::WARN, format!("Garbage collection took 14.2ms on worker #{}", (log_id % 3) + 1)),
                _ => (Tags::EXEC, format!("WASM bundle hot-reloaded for client #{}", log_id)),
            };

            live_logs.push(Line::from(vec![
                Span::styled(format!("{} ", time_str), Style::default().fg(Theme::MUTED)),
                Span::styled(format!("{} ", tag), Style::default().fg(Tags::color_for(tag))),
                Span::styled(msg, Style::default().fg(Theme::FG)),
            ]));

            log_id += 1;
            last_log_stream = Instant::now();
        }

        terminal.draw(|f| {
            let full_area = f.area();

            // Layout 3 phân vùng: Header Title (3) -> Main Tabs & Container (Min 12) -> Status Bar (1)
            let main_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3), // Top Header H1
                    Constraint::Min(12),   // Master Tabbed Container
                    Constraint::Length(1), // Bottom Status Bar
                ])
                .split(full_area);

            // 1. TOP HEADER H1
            let header = Paragraph::new(Line::from(vec![
                Span::styled("  LINEAR / VERCEL DESIGN SYSTEM", Style::default().fg(Theme::PRIMARY).add_modifier(Modifier::BOLD)),
                Span::styled(" ─ Master Industrial Component Showcase", Style::default().fg(Theme::SECONDARY)),
            ]))
            .style(Style::default().bg(Theme::BG));
            f.render_widget(header, main_chunks[0]);

            // 2. NỘI DUNG TỪNG PHÂN HỆ TAB
            match tabs.selected() {
                0 => {
                    // TAB 1: FORMS & INPUTS
                    let mut form_lines = Vec::new();
                    form_lines.push(Line::from(vec![
                        Span::styled("  Form Inputs & Keyboard Navigation", Style::default().fg(Theme::PRIMARY).add_modifier(Modifier::BOLD)),
                        Span::styled(" (Tab/↑/↓: Form Focus ─ Lists: → to enter, ← to exit ─ Radios: ←/→)", Style::default().fg(Theme::MUTED)),
                    ]));
                    form_lines.push(Line::from(""));
                    tabs.render_container(main_chunks[1], form_lines, f);

                    // Render FormManager bên trong lòng Panel
                    let inner_form_area = Rect {
                        x: main_chunks[1].x + 2,
                        y: main_chunks[1].y + 4,
                        width: main_chunks[1].width.saturating_sub(4),
                        height: main_chunks[1].height.saturating_sub(5),
                    };
                    form.render(inner_form_area, f);
                }
                1 => {
                    // TAB 2: DATA TABLE & BADGES (Read-only Data Table theo chuẩn DESIGN.md mục 5)
                    let mut lines = Vec::new();
                    lines.push(Line::from(vec![
                        Span::styled("  Deployment Workloads & Status Badges", Style::default().fg(Theme::PRIMARY).add_modifier(Modifier::BOLD)),
                        Span::styled(" (Read-only Table ─ DESIGN.md)", Style::default().fg(Theme::MUTED)),
                    ]));
                    lines.push(Line::from(""));

                    // Header bảng theo chuẩn DESIGN.md: No outer border, header separated by ─, dim separator
                    lines.push(Line::from(vec![
                        Span::styled("  NAME              STATUS         BRANCH     COMMIT    LATENCY    TIME", Style::default().fg(Theme::SECONDARY).add_modifier(Modifier::BOLD)),
                    ]));
                    lines.push(Line::from(vec![
                        Span::styled("  ─────────────────────────────────────────────────────────────────────────────", Style::default().fg(Theme::NEUTRAL_100)),
                    ]));

                    for row in &table_items {
                        lines.push(Line::from(vec![
                            Span::raw("  "),
                            Span::styled(format!("{:<16} ", row.name), Style::default().fg(Theme::FG)),
                            Span::styled(format!("{} ", row.status_icon), Style::default().fg(row.status_color)),
                            Span::styled(format!("{:<12} ", row.status_text), Style::default().fg(row.status_color)),
                            Span::styled(format!("{:<10} ", row.branch), Style::default().fg(Theme::SECONDARY)),
                            Span::styled(format!("{:<9} ", row.commit), Style::default().fg(Theme::MUTED)),
                            Span::styled(format!("{:<10} ", row.latency), Style::default().fg(Theme::ACCENT)),
                            Span::styled(row.time, Style::default().fg(Theme::MUTED)),
                        ]));
                    }

                    tabs.render_container(main_chunks[1], lines, f);

                    // Render 2 Panels / Cards (DESIGN.md mục 5) bên dưới bảng KHI VÀ CHỈ KHI đủ chiều cao (>= 24 dòng)
                    // để đảm bảo khoảng cách an toàn, không bao giờ đè lên dòng cuối của bảng
                    if main_chunks[1].height >= 24 {
                        let card_w = (main_chunks[1].width.saturating_sub(8) / 2).min(38);
                        if card_w > 20 {
                            let max_safe_y = (main_chunks[1].y + main_chunks[1].height).saturating_sub(1).min(f.area().height.saturating_sub(2));
                            let card_deploy_area = Rect {
                                x: main_chunks[1].x + 3,
                                y: main_chunks[1].y + 15,
                                width: card_w,
                                height: 7,
                            };
                            if card_deploy_area.y + card_deploy_area.height <= max_safe_y {
                                deploy_card.render(card_deploy_area, f);
                            }

                            let card_infra_area = Rect {
                                x: main_chunks[1].x + 3 + card_w + 2,
                                y: main_chunks[1].y + 15,
                                width: card_w,
                                height: 7,
                            };
                            if card_infra_area.y + card_infra_area.height <= max_safe_y {
                                infra_card.render(card_infra_area, f);
                            }
                        }
                    }
                }
                2 => {
                    // TAB 3: TASKS, PROGRESS & SHIMMER (Bố cục 2 cột công nghiệp chuẩn Vercel/Linear)
                    let mut lines = Vec::new();
                    lines.push(Line::from(vec![
                        Span::styled("  Background Task Lifecycle & Animation Engine", Style::default().fg(Theme::PRIMARY).add_modifier(Modifier::BOLD)),
                        Span::styled(" (Dual-Column Layout ─ Spinners & Metrics)", Style::default().fg(Theme::MUTED)),
                    ]));
                    lines.push(Line::from(""));
                    tabs.render_container(main_chunks[1], lines, f);

                    // Bố cục 2 cột an toàn (Dual-Column Guardrail) triệt tiêu hoàn toàn tràn biên buffer
                    let full_h = f.area().height;
                    let full_w = f.area().width;
                    let max_safe_y = (main_chunks[1].y + main_chunks[1].height).saturating_sub(1).min(full_h.saturating_sub(2));

                    let render_guard = |rect: Rect| -> bool {
                        rect.y + rect.height <= max_safe_y && rect.x + rect.width <= full_w
                    };

                    let panel_w = main_chunks[1].width.saturating_sub(6);
                    if panel_w >= 60 {
                        // Màn hình rộng >= 60: Chia 2 cột song song (Trái: Spinners, Phải: Progress Bars)
                        let col_w = (panel_w.saturating_sub(3)) / 2;
                        let left_x = main_chunks[1].x + 3;
                        let right_x = main_chunks[1].x + 3 + col_w + 3;

                        // Cột 1: 4 Spinners
                        let area_pulse = Rect { x: left_x, y: main_chunks[1].y + 3, width: col_w, height: 2 };
                        if render_guard(area_pulse) { pulse_task.render(area_pulse, f); }

                        let area_moon = Rect { x: left_x, y: main_chunks[1].y + 6, width: col_w, height: 2 };
                        if render_guard(area_moon) { moon_task.render(area_moon, f); }

                        let area_braille = Rect { x: left_x, y: main_chunks[1].y + 9, width: col_w, height: 2 };
                        if render_guard(area_braille) { braille_task.render(area_braille, f); }

                        let area_arc = Rect { x: left_x, y: main_chunks[1].y + 12, width: col_w, height: 2 };
                        if render_guard(area_arc) { arc_task.render(area_arc, f); }

                        // Cột 2: 2 Progress Bars chuẩn hóa
                        let area_line = Rect { x: right_x, y: main_chunks[1].y + 3, width: col_w, height: 2 };
                        if render_guard(area_line) { progress_line.render(area_line, f); }

                        let area_para = Rect { x: right_x, y: main_chunks[1].y + 7, width: col_w, height: 2 };
                        if render_guard(area_para) { progress_parallelogram.render(area_para, f); }

                        // Dưới cùng: Shimmer Bar trải dài
                        let area_shimmer = Rect { x: left_x, y: main_chunks[1].y + 16, width: panel_w, height: 1 };
                        if render_guard(area_shimmer) { shimmer_bar.render(area_shimmer, f); }
                    } else {
                        // Màn hình hẹp: Xếp 1 cột đơn, chỉ render các widget còn trong vùng an toàn
                        let mut curr_y = main_chunks[1].y + 3;
                        let single_tasks: [&dyn FormWidget; 6] = [
                            &pulse_task,
                            &moon_task,
                            &braille_task,
                            &arc_task,
                            &progress_line,
                            &progress_parallelogram,
                        ];

                        for t in single_tasks {
                            let area = Rect { x: main_chunks[1].x + 3, y: curr_y, width: panel_w, height: 2 };
                            if render_guard(area) {
                                t.render(area, f);
                                curr_y += 3;
                            } else {
                                break;
                            }
                        }

                        let area_shimmer = Rect { x: main_chunks[1].x + 3, y: curr_y, width: panel_w, height: 1 };
                        if render_guard(area_shimmer) {
                            shimmer_bar.render(area_shimmer, f);
                        }
                    }
                }
                3 => {
                    // TAB 4: LIVE LOGS (SMART STICKY FOLLOW & SCROLLBAR)
                    tabs.render_container(main_chunks[1], live_logs.clone(), f);
                }
                _ => {
                    // TAB 5: ICONS PALETTE & THEME MATRIX
                    let mut lines = Vec::new();
                    lines.push(Line::from(vec![
                        Span::styled("  Curated Zero-Noise Unicode 2-Cell Icons (DESIGN.md Mục 7)", Style::default().fg(Theme::PRIMARY).add_modifier(Modifier::BOLD)),
                    ]));
                    lines.push(Line::from(vec![
                        Span::styled("  ─────────────────────────────────────────────────────────────────────────────", Style::default().fg(Theme::NEUTRAL_100)),
                    ]));
                    lines.push(Line::from(""));

                    let status_icons = [
                        (Icons::SUCCESS, "Icons::SUCCESS", "✓ (U+2713)", "Theme::SUCCESS (#25A249)", Theme::SUCCESS),
                        (Icons::ERROR, "Icons::ERROR", "✗ (U+2716)", "Theme::ERROR   (#DA1E28)", Theme::ERROR),
                        (Icons::WARNING, "Icons::WARNING", "! (U+0021)", "Theme::WARNING (#F1C21B)", Theme::WARNING),
                        (Icons::RUN, "Icons::RUN", "▶ (U+25B6)", "Theme::ACCENT  (#0070F3)", Theme::ACCENT),
                        (Icons::POINTER_RIGHT, "Icons::POINTER_RIGHT", "► (U+25BA)", "Theme::MUTED   (#8D8D8D)", Theme::MUTED),
                        (Icons::BUILD, "Icons::BUILD", "⛯ (U+26EF)", "Theme::PRIMARY (#4589FF)", Theme::PRIMARY),
                        (Icons::INFO, "Icons::INFO", "i (U+0069)", "Theme::ACCENT  (#0070F3)", Theme::ACCENT),
                        (Icons::PAUSE, "Icons::PAUSE", "॥ (U+0965)", "Theme::MUTED   (#8D8D8D)", Theme::MUTED),
                        (Icons::STOP, "Icons::STOP", "■ (U+25A0)", "Theme::ERROR   (#DA1E28)", Theme::ERROR),
                        (Icons::SPARKLE_FILLED, "Icons::SPARKLE_FILLED", "✦ (U+2726)", "Theme::WARNING (#F1C21B)", Theme::WARNING),
                        (Icons::STAR_OUTLINE, "Icons::STAR_OUTLINE", "⚝ (U+269D)", "Theme::PRIMARY (#4589FF)", Theme::PRIMARY),
                        (Icons::SNOWFLAKE, "Icons::SNOWFLAKE", "❅ (U+2745)", "Theme::ACCENT  (#0070F3)", Theme::ACCENT),
                        (Icons::BRANCH, "Icons::BRANCH", "⤷ (U+21B3)", "Theme::SECONDARY (#A8A8A8)", Theme::SECONDARY),
                    ];

                    for (glyph, const_name, unicode_char, color_name, color) in status_icons {
                        lines.push(Line::from(vec![
                            Span::styled(format!("    {}", glyph), Style::default().fg(color)),
                            Span::styled(format!("{:<22} ", const_name), Style::default().fg(Theme::FG)),
                            Span::styled(format!("{:<15} ", unicode_char), Style::default().fg(Theme::SECONDARY)),
                            Span::styled(color_name, Style::default().fg(color)),
                        ]));
                    }

                    // Bộ hình học an toàn (Safe Geometric Glyphs - Windows Terminal Verified)
                    lines.push(Line::from(""));
                    lines.push(Line::from(vec![
                        Span::styled("  Safe Geometric Glyphs Matrix (Windows/Linux Terminal Zero-Clipping)", Style::default().fg(Theme::PRIMARY).add_modifier(Modifier::BOLD)),
                    ]));
                    lines.push(Line::from(vec![
                        Span::styled("  ─────────────────────────────────────────────────────────────────────────────", Style::default().fg(Theme::NEUTRAL_100)),
                    ]));
                    lines.push(Line::from(""));

                    let geometric_icons = [
                        // 1. Squares & Checkboxes
                        (Icons::SQUARE_FILLED, "Icons::SQUARE_FILLED", "■ (U+25A0)", "Theme::SUCCESS (#25A249)", Theme::SUCCESS),
                        (Icons::SQUARE_EMPTY, "Icons::SQUARE_EMPTY", "□ (U+25A1)", "Theme::MUTED   (#8D8D8D)", Theme::MUTED),
                        (Icons::SQUARE_MEDIUM_FILLED, "Icons::SQUARE_MEDIUM_FILLED", "◼ (U+25FC)", "Theme::PRIMARY (#4589FF)", Theme::PRIMARY),
                        (Icons::SQUARE_MEDIUM_EMPTY, "Icons::SQUARE_MEDIUM_EMPTY", "◻ (U+25FB)", "Theme::SECONDARY (#A8A8A8)", Theme::SECONDARY),
                        (Icons::SQUARE_SMALL_FILLED, "Icons::SQUARE_SMALL_FILLED", "▪ (U+25AA)", "Theme::MUTED   (#8D8D8D)", Theme::MUTED),
                        (Icons::SQUARE_DOTTED, "Icons::SQUARE_DOTTED", "⬚ (U+2B1A)", "Theme::SECONDARY (#A8A8A8)", Theme::SECONDARY),
                        (Icons::BOX_SHADOW, "Icons::BOX_SHADOW", "❒ (U+2752)", "Theme::CYAN    (#50E3C2)", Theme::CYAN),
                        // 2. Circles & Activity States
                        (Icons::CIRCLE_FILLED, "Icons::CIRCLE_FILLED", "● (U+25CF)", "Theme::ACCENT  (#0070F3)", Theme::ACCENT),
                        (Icons::CIRCLE_EMPTY, "Icons::CIRCLE_EMPTY", "○ (U+25CB)", "Theme::MUTED   (#8D8D8D)", Theme::MUTED),
                        (Icons::CIRCLE_DOTTED, "Icons::CIRCLE_DOTTED", "◌ (U+25CC)", "Theme::SECONDARY (#A8A8A8)", Theme::SECONDARY),
                        (Icons::CIRCLE_TARGET, "Icons::CIRCLE_TARGET", "◉ (U+25C9)", "Theme::CYAN    (#50E3C2)", Theme::CYAN),
                        (Icons::CIRCLE_HALF_LEFT, "Icons::CIRCLE_HALF_LEFT", "◐ (U+25D0)", "Theme::CYAN    (#50E3C2)", Theme::CYAN),
                        (Icons::CIRCLE_HALF_BOTTOM, "Icons::CIRCLE_HALF_BOTTOM", "◒ (U+25D2)", "Theme::CYAN    (#50E3C2)", Theme::CYAN),
                        (Icons::CIRCLE_HALF_RIGHT, "Icons::CIRCLE_HALF_RIGHT", "◑ (U+25D1)", "Theme::CYAN    (#50E3C2)", Theme::CYAN),
                        (Icons::CIRCLE_HALF_TOP, "Icons::CIRCLE_HALF_TOP", "◓ (U+25D3)", "Theme::CYAN    (#50E3C2)", Theme::CYAN),
                        // 2b. Quadrant Circular Arcs (4-Frame Arc Spinner)
                        (Icons::ARC_TOP_LEFT, "Icons::ARC_TOP_LEFT", "◜ (U+25DC)", "Theme::EMERALD (#10B981)", Theme::EMERALD),
                        (Icons::ARC_TOP_RIGHT, "Icons::ARC_TOP_RIGHT", "◝ (U+25DD)", "Theme::EMERALD (#10B981)", Theme::EMERALD),
                        (Icons::ARC_BOTTOM_RIGHT, "Icons::ARC_BOTTOM_RIGHT", "◞ (U+25DE)", "Theme::EMERALD (#10B981)", Theme::EMERALD),
                        (Icons::ARC_BOTTOM_LEFT, "Icons::ARC_BOTTOM_LEFT", "◟ (U+25DF)", "Theme::EMERALD (#10B981)", Theme::EMERALD),
                        // 3. Pointers & Triangles
                        (Icons::TRIANGLE_UP, "Icons::TRIANGLE_UP", "▲ (U+25B2)", "Theme::WARNING (#F1C21B)", Theme::WARNING),
                        (Icons::TRIANGLE_DOWN, "Icons::TRIANGLE_DOWN", "▼ (U+25BC)", "Theme::ERROR   (#DA1E28)", Theme::ERROR),
                        (Icons::TRIANGLE_RIGHT_SMALL, "Icons::TRIANGLE_RIGHT_SMALL", "▸ (U+25B8)", "Theme::INDIGO  (#5E6AD2)", Theme::INDIGO),
                        (Icons::POINTER, "Icons::POINTER", "▹ (U+25B9)", "Theme::PRIMARY (#4589FF)", Theme::PRIMARY),
                        (Icons::ARROWHEAD_RIGHT, "Icons::ARROWHEAD_RIGHT", "⮞ (U+2B9E)", "Theme::ACCENT  (#0070F3)", Theme::ACCENT),
                        // 4. Diamonds
                        (Icons::DIAMOND_FILLED, "Icons::DIAMOND_FILLED", "◆ (U+25C6)", "Theme::PURPLE  (#7928CA)", Theme::PURPLE),
                        (Icons::DIAMOND_EMPTY, "Icons::DIAMOND_EMPTY", "◇ (U+25C7)", "Theme::SECONDARY (#A8A8A8)", Theme::SECONDARY),
                        // 5. Media, Navigation & System
                        (Icons::MUSIC, "Icons::MUSIC", "♪ (U+266A)", "Theme::CYAN    (#50E3C2)", Theme::CYAN),
                        (Icons::MUSIC_DOUBLE, "Icons::MUSIC_DOUBLE", "♫ (U+266B)", "Theme::PURPLE  (#7928CA)", Theme::PURPLE),
                        (Icons::TAB, "Icons::TAB", "⇥ (U+21E5)", "Theme::ACCENT  (#0070F3)", Theme::ACCENT),
                        (Icons::MENU, "Icons::MENU", "≡ (U+2261)", "Theme::SECONDARY (#A8A8A8)", Theme::SECONDARY),
                        (Icons::HOURGLASS, "Icons::HOURGLASS", "⧗ (U+29D7)", "Theme::ORANGE  (#FF8800)", Theme::ORANGE),
                        (Icons::SQUARE_CONTAINED, "Icons::SQUARE_CONTAINED", "▣ (U+25A3)", "Theme::EMERALD (#10B981)", Theme::EMERALD),
                        // 6. Technical, Hardware & Targeting
                        (Icons::PIN, "Icons::PIN", "⚲ (U+26B2)", "Theme::CRITICAL (#FF0055)", Theme::CRITICAL),
                        (Icons::SPARK, "Icons::SPARK", "⌁ (U+2301)", "Theme::WARNING (#F1C21B)", Theme::WARNING),
                        (Icons::BULLSEYE, "Icons::BULLSEYE", "◎ (U+25CE)", "Theme::CYAN    (#50E3C2)", Theme::CYAN),
                        (Icons::CROSSHAIR, "Icons::CROSSHAIR", "⌖ (U+2316)", "Theme::SKY     (#38BDF8)", Theme::SKY),
                        (Icons::APPROX, "Icons::APPROX", "≈ (U+2248)", "Theme::SECONDARY (#A8A8A8)", Theme::SECONDARY),
                        (Icons::SEARCH, "Icons::SEARCH", "⌕ (U+2315)", "Theme::CYAN    (#50E3C2)", Theme::CYAN),
                        // 7. Directional Arrows
                        (Icons::ARROW_UP, "Icons::ARROW_UP", "↑ (U+2191)", "Theme::SUCCESS (#25A249)", Theme::SUCCESS),
                        (Icons::ARROW_DOWN, "Icons::ARROW_DOWN", "↓ (U+2193)", "Theme::ERROR   (#DA1E28)", Theme::ERROR),
                        (Icons::ARROW_LEFT, "Icons::ARROW_LEFT", "← (U+2190)", "Theme::MUTED   (#8D8D8D)", Theme::MUTED),
                        (Icons::ARROW_RIGHT, "Icons::ARROW_RIGHT", "→ (U+2192)", "Theme::PRIMARY (#4589FF)", Theme::PRIMARY),
                        (Icons::ARROW_UP_DOWN, "Icons::ARROW_UP_DOWN", "↕ (U+2195)", "Theme::SECONDARY (#A8A8A8)", Theme::SECONDARY),
                        (Icons::ARROW_LEFT_RIGHT, "Icons::ARROW_LEFT_RIGHT", "↔ (U+2194)", "Theme::SECONDARY (#A8A8A8)", Theme::SECONDARY),
                        (Icons::ENTER, "Icons::ENTER", "↵ (U+21B5)", "Theme::ACCENT  (#0070F3)", Theme::ACCENT),
                        (Icons::RELOAD, "Icons::RELOAD", "↻ (U+21BB)", "Theme::CYAN    (#50E3C2)", Theme::CYAN),
                        (Icons::UNDO, "Icons::UNDO", "↺ (U+21BA)", "Theme::ORANGE  (#FF8800)", Theme::ORANGE),
                        (Icons::ARROW_DOUBLE_RIGHT, "Icons::ARROW_DOUBLE_RIGHT", "⇒ (U+21D2)", "Theme::PRIMARY (#4589FF)", Theme::PRIMARY),
                        (Icons::ARROW_FAST_RIGHT, "Icons::ARROW_FAST_RIGHT", "↠ (U+21A0)", "Theme::ACCENT  (#0070F3)", Theme::ACCENT),
                        (Icons::ARROW_HOOK_RIGHT, "Icons::ARROW_HOOK_RIGHT", "↪ (U+21AA)", "Theme::CYAN    (#50E3C2)", Theme::CYAN),
                        (Icons::EXTERNAL_LINK, "Icons::EXTERNAL_LINK", "↗ (U+2197)", "Theme::WARNING (#F1C21B)", Theme::WARNING),
                        // 7b. Exchange & Harpoons (Block U+2190..U+21FF)
                        (Icons::ARROW_SWAP, "Icons::ARROW_SWAP", "⇄ (U+21C4)", "Theme::CYAN    (#50E3C2)", Theme::CYAN),
                        (Icons::ARROW_TRANSFER, "Icons::ARROW_TRANSFER", "⇆ (U+21C6)", "Theme::SKY     (#38BDF8)", Theme::SKY),
                        (Icons::ARROW_DUPLEX_VERT, "Icons::ARROW_DUPLEX_VERT", "⇅ (U+21C5)", "Theme::PRIMARY (#4589FF)", Theme::PRIMARY),
                        (Icons::ARROW_DUPLEX_VERT_REV, "Icons::ARROW_DUPLEX_REV", "⇵ (U+21F5)", "Theme::SECONDARY (#A8A8A8)", Theme::SECONDARY),
                        (Icons::HARPOON_LEFT_RIGHT, "Icons::HARPOON_LEFT_RIGHT", "⇋ (U+21CB)", "Theme::EMERALD (#10B981)", Theme::EMERALD),
                        (Icons::HARPOON_RIGHT_LEFT, "Icons::HARPOON_RIGHT_LEFT", "⇌ (U+21CC)", "Theme::SUCCESS (#25A249)", Theme::SUCCESS),
                        // 7c. Supplemental Arrows-B (Block U+2900..U+297F)
                        (Icons::HARPOON_UP_DOWN, "Icons::HARPOON_UP_DOWN", "⥮ (U+296E)", "Theme::CYAN    (#50E3C2)", Theme::CYAN),
                        (Icons::HARPOON_DOWN_UP, "Icons::HARPOON_DOWN_UP", "⥯ (U+296F)", "Theme::CYAN    (#50E3C2)", Theme::CYAN),
                        (Icons::ARROW_TO_BAR_TOP, "Icons::ARROW_TO_BAR_TOP", "⤒ (U+2912)", "Theme::ACCENT  (#0070F3)", Theme::ACCENT),
                        (Icons::ARROW_TO_BAR_BOTTOM, "Icons::ARROW_TO_BAR_BOT", "⤓ (U+2913)", "Theme::ACCENT  (#0070F3)", Theme::ACCENT),
                        (Icons::ARROW_DIAGONAL_NW_SE, "Icons::ARROW_DIAG_NW_SE", "⤡ (U+2921)", "Theme::SKY     (#38BDF8)", Theme::SKY),
                        (Icons::ARROW_DIAGONAL_NE_SW, "Icons::ARROW_DIAG_NE_SW", "⤢ (U+2922)", "Theme::SKY     (#38BDF8)", Theme::SKY),
                        (Icons::ARROW_CURVE_DOWN_LEFT, "Icons::ARROW_CURVE_DL", "⤶ (U+2936)", "Theme::SECONDARY (#A8A8A8)", Theme::SECONDARY),
                        (Icons::ARROW_CURVE_DOWN_RIGHT, "Icons::ARROW_CURVE_DR", "⤷ (U+2937)", "Theme::SECONDARY (#A8A8A8)", Theme::SECONDARY),
                        // 8. Flags & Milestones
                        (Icons::FLAG_FILLED, "Icons::FLAG_FILLED", "⚑ (U+2691)", "Theme::CRITICAL (#FF0055)", Theme::CRITICAL),
                        (Icons::FLAG_EMPTY, "Icons::FLAG_EMPTY", "⚐ (U+2690)", "Theme::SECONDARY (#A8A8A8)", Theme::SECONDARY),
                        // 9. Celestial, Stars & Links
                        (Icons::SUN, "Icons::SUN", "☼ (U+263C)", "Theme::WARNING (#F1C21B)", Theme::WARNING),
                        (Icons::STAR_FILLED, "Icons::STAR_FILLED", "★ (U+2605)", "Theme::WARNING (#F1C21B)", Theme::WARNING),
                        (Icons::STAR_EMPTY, "Icons::STAR_EMPTY", "☆ (U+2606)", "Theme::MUTED   (#8D8D8D)", Theme::MUTED),
                        (Icons::LIGHTNING, "Icons::LIGHTNING", "☇ (U+2607)", "Theme::CYAN    (#50E3C2)", Theme::CYAN),
                        (Icons::NODE_LINK, "Icons::NODE_LINK", "☌ (U+260C)", "Theme::PRIMARY (#4589FF)", Theme::PRIMARY),
                        (Icons::PEER_LINK, "Icons::PEER_LINK", "☍ (U+260D)", "Theme::PURPLE  (#7928CA)", Theme::PURPLE),
                        // 10. Ornaments & Angle Brackets
                        (Icons::BRACKET_HEAVY_LEFT, "Icons::BRACKET_HEAVY_L", "❰ (U+2770)", "Theme::ACCENT  (#0070F3)", Theme::ACCENT),
                        (Icons::BRACKET_HEAVY_RIGHT, "Icons::BRACKET_HEAVY_R", "❱ (U+2771)", "Theme::ACCENT  (#0070F3)", Theme::ACCENT),
                        (Icons::BRACKET_MEDIUM_LEFT, "Icons::BRACKET_MEDIUM_L", "❬ (U+276C)", "Theme::SECONDARY (#A8A8A8)", Theme::SECONDARY),
                        (Icons::BRACKET_MEDIUM_RIGHT, "Icons::BRACKET_MEDIUM_R", "❭ (U+276D)", "Theme::SECONDARY (#A8A8A8)", Theme::SECONDARY),
                        (Icons::QUOTE_ANGLE_LEFT, "Icons::QUOTE_ANGLE_L", "❮ (U+276E)", "Theme::WARNING (#F1C21B)", Theme::WARNING),
                        (Icons::QUOTE_ANGLE_RIGHT, "Icons::QUOTE_ANGLE_R", "❯ (U+276F)", "Theme::WARNING (#F1C21B)", Theme::WARNING),
                    ];

                    for (glyph, const_name, unicode_char, color_name, color) in geometric_icons {
                        lines.push(Line::from(vec![
                            Span::styled(format!("    {}", glyph), Style::default().fg(color)),
                            Span::styled(format!("{:<26} ", const_name), Style::default().fg(Theme::FG)),
                            Span::styled(format!("{:<15} ", unicode_char), Style::default().fg(Theme::SECONDARY)),
                            Span::styled(color_name, Style::default().fg(color)),
                        ]));
                    }

                    // Showcase ứng dụng trực tiếp các glyph hình học vào UI
                    lines.push(Line::from(""));
                    lines.push(Line::from(vec![
                        Span::styled("  Live Geometric & Technical UI Components Demo", Style::default().fg(Theme::PRIMARY).add_modifier(Modifier::BOLD)),
                    ]));
                    lines.push(Line::from(vec![
                        Span::styled("  ─────────────────────────────────────────────────────────────────────────────", Style::default().fg(Theme::NEUTRAL_100)),
                    ]));
                    lines.push(Line::from(""));

                    lines.push(Line::from(vec![
                        Span::styled("    [States/Orbit] ", Style::default().fg(Theme::SECONDARY)),
                        Span::styled(format!(" {}", Icons::STATE_IDLE), Style::default().fg(Theme::SECONDARY)),
                        Span::styled("Idle  ", Style::default().fg(Theme::MUTED)),
                        Span::styled(Icons::STATE_INACTIVE, Style::default().fg(Theme::MUTED)),
                        Span::styled("Off  ", Style::default().fg(Theme::MUTED)),
                        Span::styled(Icons::STATE_ACTIVE, Style::default().fg(Theme::EMERALD)),
                        Span::styled("Active │ ", Style::default().fg(Theme::EMERALD)),
                        Span::styled("4-Phase Orbit: ", Style::default().fg(Theme::SECONDARY)),
                        Span::styled(Icons::CIRCLE_HALF_LEFT, Style::default().fg(Theme::CYAN)),
                        Span::styled(Icons::CIRCLE_HALF_BOTTOM, Style::default().fg(Theme::CYAN)),
                        Span::styled(Icons::CIRCLE_HALF_RIGHT, Style::default().fg(Theme::CYAN)),
                        Span::styled(Icons::CIRCLE_HALF_TOP, Style::default().fg(Theme::CYAN)),
                    ]));

                    lines.push(Line::from(vec![
                        Span::styled("    [Progress/Bar] ", Style::default().fg(Theme::SECONDARY)),
                        Span::styled("▰▰▰▱▱ ", Style::default().fg(Theme::CYAN)),
                        Span::styled("Parallelogram │ ", Style::default().fg(Theme::MUTED)),
                        Span::styled("━━━── ", Style::default().fg(Theme::EMERALD)),
                        Span::styled("Line (Vercel Minimal)", Style::default().fg(Theme::MUTED)),
                    ]));

                    lines.push(Line::from(vec![
                        Span::styled("    [Box/Angles]   ", Style::default().fg(Theme::SECONDARY)),
                        Span::styled(format!(" {}", Icons::BOX_SHADOW), Style::default().fg(Theme::CYAN)),
                        Span::styled("Box Container   ", Style::default().fg(Theme::FG)),
                        Span::styled(Icons::BRACKET_HEAVY_LEFT, Style::default().fg(Theme::ACCENT)),
                        Span::styled("HEAVY", Style::default().fg(Theme::PRIMARY).add_modifier(Modifier::BOLD)),
                        Span::styled(format!("{}  ", Icons::BRACKET_HEAVY_RIGHT), Style::default().fg(Theme::ACCENT)),
                        Span::styled(Icons::BRACKET_MEDIUM_LEFT, Style::default().fg(Theme::SECONDARY)),
                        Span::styled("MEDIUM", Style::default().fg(Theme::FG)),
                        Span::styled(format!("{}  ", Icons::BRACKET_MEDIUM_RIGHT), Style::default().fg(Theme::SECONDARY)),
                        Span::styled(Icons::QUOTE_ANGLE_LEFT, Style::default().fg(Theme::WARNING)),
                        Span::styled("QUOTE", Style::default().fg(Theme::WARNING)),
                        Span::styled(Icons::QUOTE_ANGLE_RIGHT, Style::default().fg(Theme::WARNING)),
                    ]));

                    lines.push(Line::from(vec![
                        Span::styled("    [Stars/Sun]  ", Style::default().fg(Theme::SECONDARY)),
                        Span::styled(format!(" {}", Icons::SUN), Style::default().fg(Theme::WARNING)),
                        Span::styled("Daylight Engine Active    ", Style::default().fg(Theme::FG)),
                        Span::styled(Icons::STAR_FILLED, Style::default().fg(Theme::WARNING)),
                        Span::styled(Icons::STAR_FILLED, Style::default().fg(Theme::WARNING)),
                        Span::styled(Icons::STAR_FILLED, Style::default().fg(Theme::WARNING)),
                        Span::styled(Icons::STAR_EMPTY, Style::default().fg(Theme::MUTED)),
                        Span::styled(Icons::STAR_EMPTY, Style::default().fg(Theme::MUTED)),
                        Span::styled(" Rating (3/5)", Style::default().fg(Theme::MUTED)),
                    ]));

                    lines.push(Line::from(vec![
                        Span::styled("    [Links/Node] ", Style::default().fg(Theme::SECONDARY)),
                        Span::styled(format!(" {}", Icons::LIGHTNING), Style::default().fg(Theme::CYAN)),
                        Span::styled("Fast Flash Trigger        ", Style::default().fg(Theme::FG)),
                        Span::styled(Icons::NODE_LINK, Style::default().fg(Theme::PRIMARY)),
                        Span::styled("Socket Pair  │  ", Style::default().fg(Theme::MUTED)),
                        Span::styled(Icons::PEER_LINK, Style::default().fg(Theme::PURPLE)),
                        Span::styled("P2P Mesh Link", Style::default().fg(Theme::PURPLE)),
                    ]));

                    lines.push(Line::from(vec![
                        Span::styled("    [Arrows/Stream]", Style::default().fg(Theme::SECONDARY)),
                        Span::styled(format!(" {}", Icons::ARROW_SWAP), Style::default().fg(Theme::CYAN)),
                        Span::styled("Swap  ", Style::default().fg(Theme::FG)),
                        Span::styled(Icons::HARPOON_EXCHANGE, Style::default().fg(Theme::EMERALD)),
                        Span::styled("Sync  ", Style::default().fg(Theme::FG)),
                        Span::styled(Icons::HARPOON_UP_DOWN, Style::default().fg(Theme::SKY)),
                        Span::styled("Duplex Stream │ ", Style::default().fg(Theme::SKY)),
                        Span::styled("Branch: ", Style::default().fg(Theme::SECONDARY)),
                        Span::styled(Icons::BRANCH, Style::default().fg(Theme::SECONDARY)),
                    ]));

                    lines.push(Line::from(vec![
                        Span::styled("    [Flags/Mark] ", Style::default().fg(Theme::SECONDARY)),
                        Span::styled(format!(" {}", Icons::FLAG_FILLED), Style::default().fg(Theme::CRITICAL)),
                        Span::styled("Release Milestone v1.0   ", Style::default().fg(Theme::FG)),
                        Span::styled(Icons::FLAG_EMPTY, Style::default().fg(Theme::MUTED)),
                        Span::styled("Roadmap Horizon v2.0", Style::default().fg(Theme::MUTED)),
                    ]));

                    lines.push(Line::from(vec![
                        Span::styled("    [Arrows/Flow]", Style::default().fg(Theme::SECONDARY)),
                        Span::styled(format!(" {}", Icons::ARROW_UP), Style::default().fg(Theme::SUCCESS)),
                        Span::styled("Metric +14%   ", Style::default().fg(Theme::SUCCESS)),
                        Span::styled(Icons::ARROW_DOWN, Style::default().fg(Theme::ERROR)),
                        Span::styled("Memory -3%    ", Style::default().fg(Theme::ERROR)),
                        Span::styled(Icons::RELOAD, Style::default().fg(Theme::CYAN)),
                        Span::styled("Sync Engine   ", Style::default().fg(Theme::CYAN)),
                        Span::styled(Icons::ENTER, Style::default().fg(Theme::ACCENT)),
                        Span::styled("Submit (Enter)", Style::default().fg(Theme::ACCENT)),
                    ]));

                    lines.push(Line::from(vec![
                        Span::styled("    [Sensor/HW]  ", Style::default().fg(Theme::SECONDARY)),
                        Span::styled(format!(" {}", Icons::SPARK), Style::default().fg(Theme::WARNING)),
                        Span::styled("High Voltage: 3.3V Active ", Style::default().fg(Theme::FG)),
                        Span::styled(Icons::PIN, Style::default().fg(Theme::CRITICAL)),
                        Span::styled("GPIO Pin 12 (Anchor)", Style::default().fg(Theme::CRITICAL)),
                    ]));

                    lines.push(Line::from(vec![
                        Span::styled("    [Radar/Aim]  ", Style::default().fg(Theme::SECONDARY)),
                        Span::styled(format!(" {}", Icons::BULLSEYE), Style::default().fg(Theme::CYAN)),
                        Span::styled("Radar Lock: Focused       ", Style::default().fg(Theme::FG)),
                        Span::styled(Icons::CROSSHAIR, Style::default().fg(Theme::SKY)),
                        Span::styled("GPS Target Calibrated", Style::default().fg(Theme::SKY)),
                        Span::styled(format!("  {} 12.4ms", Icons::APPROX), Style::default().fg(Theme::MUTED)),
                    ]));

                    lines.push(Line::from(vec![
                        Span::styled("    [Media/Audio]", Style::default().fg(Theme::SECONDARY)),
                        Span::styled(format!(" {}", Icons::MUSIC), Style::default().fg(Theme::CYAN)),
                        Span::styled("Voice Dub Track 01        ", Style::default().fg(Theme::FG)),
                        Span::styled(Icons::MUSIC_DOUBLE, Style::default().fg(Theme::PURPLE)),
                        Span::styled("Master Stereo BGM (Active)", Style::default().fg(Theme::FG)),
                    ]));

                    lines.push(Line::from(vec![
                        Span::styled("    [Nav/Menu]   ", Style::default().fg(Theme::SECONDARY)),
                        Span::styled(format!(" {}", Icons::MENU), Style::default().fg(Theme::SECONDARY)),
                        Span::styled("Root Actions Menu         ", Style::default().fg(Theme::FG)),
                        Span::styled(Icons::TAB, Style::default().fg(Theme::ACCENT)),
                        Span::styled("Tab / Next Widget Field", Style::default().fg(Theme::ACCENT)),
                    ]));

                    lines.push(Line::from(vec![
                        Span::styled("    [Wait/Box]   ", Style::default().fg(Theme::SECONDARY)),
                        Span::styled(format!(" {}", Icons::HOURGLASS), Style::default().fg(Theme::ORANGE)),
                        Span::styled("Rendering Pipeline (42s)  ", Style::default().fg(Theme::FG)),
                        Span::styled(Icons::SQUARE_CONTAINED, Style::default().fg(Theme::EMERALD)),
                        Span::styled("Nested Container Frame", Style::default().fg(Theme::EMERALD)),
                    ]));

                    lines.push(Line::from(vec![
                        Span::styled("    [Squares]    ", Style::default().fg(Theme::SECONDARY)),
                        Span::styled(format!(" {}", Icons::SQUARE_FILLED), Style::default().fg(Theme::SUCCESS)),
                        Span::styled("Cluster Isolation: Active ", Style::default().fg(Theme::FG)),
                        Span::styled(Icons::SQUARE_EMPTY, Style::default().fg(Theme::MUTED)),
                        Span::styled("WebGPU Pipeline: Idle     ", Style::default().fg(Theme::MUTED)),
                        Span::styled(Icons::SQUARE_SMALL_FILLED, Style::default().fg(Theme::SECONDARY)),
                        Span::styled("Sub-process worker", Style::default().fg(Theme::SECONDARY)),
                    ]));

                    lines.push(Line::from(vec![
                        Span::styled("    [Radio/Node] ", Style::default().fg(Theme::SECONDARY)),
                        Span::styled(format!(" {}", Icons::CIRCLE_TARGET), Style::default().fg(Theme::CYAN)),
                        Span::styled("Leader Node (Coordinator) ", Style::default().fg(Theme::FG)),
                        Span::styled(Icons::CIRCLE_EMPTY, Style::default().fg(Theme::MUTED)),
                        Span::styled("Standby Replica #01       ", Style::default().fg(Theme::MUTED)),
                        Span::styled(Icons::CIRCLE_FILLED, Style::default().fg(Theme::ACCENT)),
                        Span::styled("Quorum Acked", Style::default().fg(Theme::ACCENT)),
                    ]));

                    lines.push(Line::from(vec![
                        Span::styled("    [Breadcrumb] ", Style::default().fg(Theme::SECONDARY)),
                        Span::styled(" Workspace ", Style::default().fg(Theme::MUTED)),
                        Span::styled(Icons::TRIANGLE_RIGHT_SMALL, Style::default().fg(Theme::PRIMARY)),
                        Span::styled("cli_gui_form ", Style::default().fg(Theme::MUTED)),
                        Span::styled(Icons::TRIANGLE_RIGHT_SMALL, Style::default().fg(Theme::PRIMARY)),
                        Span::styled("src ", Style::default().fg(Theme::MUTED)),
                        Span::styled(Icons::TRIANGLE_RIGHT_SMALL, Style::default().fg(Theme::PRIMARY)),
                        Span::styled("icons.rs", Style::default().fg(Theme::PRIMARY).add_modifier(Modifier::BOLD)),
                    ]));

                    lines.push(Line::from(vec![
                        Span::styled("    [Sort/Order] ", Style::default().fg(Theme::SECONDARY)),
                        Span::styled(" Sort: Latency ", Style::default().fg(Theme::FG)),
                        Span::styled(Icons::TRIANGLE_UP, Style::default().fg(Theme::WARNING)),
                        Span::styled("Ascending (18ms)  │  Error Rate ", Style::default().fg(Theme::MUTED)),
                        Span::styled(Icons::TRIANGLE_DOWN, Style::default().fg(Theme::ERROR)),
                        Span::styled("Descending (0.01%)", Style::default().fg(Theme::ERROR)),
                    ]));

                    lines.push(Line::from(vec![
                        Span::styled("    [Diamonds]   ", Style::default().fg(Theme::SECONDARY)),
                        Span::styled(format!(" {}", Icons::DIAMOND_FILLED), Style::default().fg(Theme::PURPLE)),
                        Span::styled("Tier 1: Mission-Critical  ", Style::default().fg(Theme::FG)),
                        Span::styled(Icons::DIAMOND_EMPTY, Style::default().fg(Theme::SECONDARY)),
                        Span::styled("Tier 2: Asynchronous Backlog", Style::default().fg(Theme::SECONDARY)),
                    ]));

                    // Fixed-Width Tag System Showcase (DESIGN.md Mục 7.1)
                    lines.push(Line::from(""));
                    lines.push(Line::from(vec![
                        Span::styled("  Fixed-Width Tag System Matrix (6-Cell [XXXX] & 3-Cell [X])", Style::default().fg(Theme::PRIMARY).add_modifier(Modifier::BOLD)),
                    ]));
                    lines.push(Line::from(vec![
                        Span::styled("  ─────────────────────────────────────────────────────────────────────────────", Style::default().fg(Theme::NEUTRAL_100)),
                    ]));
                    lines.push(Line::from(""));

                    let tag_examples = [
                        (Tags::INFO, "Tags::INFO", "Information notice", Tags::color_for(Tags::INFO)),
                        (Tags::WARN, "Tags::WARN", "Warning threshold", Tags::color_for(Tags::WARN)),
                        (Tags::FAIL, "Tags::FAIL", "Fatal error / Panic", Tags::color_for(Tags::FAIL)),
                        (Tags::PASS, "Tags::PASS", "Test passed / Verified", Tags::color_for(Tags::PASS)),
                        (Tags::DONE, "Tags::DONE", "Task job completed", Tags::color_for(Tags::DONE)),
                        (Tags::WAIT, "Tags::WAIT", "Queued in scheduler", Tags::color_for(Tags::WAIT)),
                        (Tags::EXEC, "Tags::EXEC", "Process execution", Tags::color_for(Tags::EXEC)),
                        (Tags::CONF, "Tags::CONF", "Config file parsed", Tags::color_for(Tags::CONF)),
                        (Tags::DEVS, "Tags::DEVS", "Hardware / Dev env", Tags::color_for(Tags::DEVS)),
                        (Tags::NETW, "Tags::NETW", "Socket connection", Tags::color_for(Tags::NETW)),
                        (Tags::AUDI, "Tags::AUDI", "Audio stream dubbing", Tags::color_for(Tags::AUDI)),
                    ];

                    for (tag, name, desc, color) in tag_examples {
                        lines.push(Line::from(vec![
                            Span::styled(format!("    {} ", tag), Style::default().fg(color).add_modifier(Modifier::BOLD)),
                            Span::styled(format!("{:<14} ", name), Style::default().fg(Theme::FG)),
                            Span::styled(desc, Style::default().fg(Theme::SECONDARY)),
                        ]));
                    }

                    lines.push(Line::from(""));
                    lines.push(Line::from(vec![
                        Span::styled("    [Compact 3-Cell] ", Style::default().fg(Theme::SECONDARY)),
                        Span::styled(format!("{} Checkbox  ", Tags::CHECKED), Style::default().fg(Theme::SUCCESS)),
                        Span::styled(format!("{} Unchecked  ", Tags::UNCHECKED), Style::default().fg(Theme::MUTED)),
                        Span::styled(format!("{} Radio Active  ", Tags::RADIO_ACTIVE), Style::default().fg(Theme::ACCENT)),
                        Span::styled(format!("{} Folder Closed", Tags::EXPAND), Style::default().fg(Theme::PRIMARY)),
                    ]));

                    // Bảng màu mở rộng Extended Palette Swatches
                    lines.push(Line::from(""));
                    lines.push(Line::from(vec![
                        Span::styled("  Extended Palette Swatches (Multi-Role System)", Style::default().fg(Theme::PRIMARY).add_modifier(Modifier::BOLD)),
                    ]));
                    lines.push(Line::from(vec![
                        Span::styled("  ─────────────────────────────────────────────────────────────────────────────", Style::default().fg(Theme::NEUTRAL_100)),
                    ]));
                    lines.push(Line::from(""));

                    let swatches = [
                        ("Theme::CYAN", "■ #50E3C2", Theme::CYAN, "Network, API Latency, Endpoints"),
                        ("Theme::PURPLE", "■ #7928CA", Theme::PURPLE, "AI Engine, Neural, GraphQL"),
                        ("Theme::MAGENTA", "■ #F81CE5", Theme::MAGENTA, "Auth Tokens, Webhooks, Secrets"),
                        ("Theme::ORANGE", "■ #FF8800", Theme::ORANGE, "Queues, Build Pipelines, Workers"),
                        ("Theme::INDIGO", "■ #5E6AD2", Theme::INDIGO, "Branches, PRs, Linear Tasks"),
                        ("Theme::EMERALD", "■ #10B981", Theme::EMERALD, "Healthy Uptime, In-Memory DBs"),
                        ("Theme::SKY", "■ #38BDF8", Theme::SKY, "Cloud Infra, Docker, Kubernetes"),
                        ("Theme::CRITICAL", "■ #FF0055", Theme::CRITICAL, "Fatal Panics, Immediate Alerts"),
                    ];

                    for (name, block, color, desc) in swatches {
                        lines.push(Line::from(vec![
                            Span::raw("    "),
                            Span::styled(format!("{:<12} ", block), Style::default().fg(color)),
                            Span::styled(format!("{:<18} ", name), Style::default().fg(Theme::FG)),
                            Span::styled(desc, Style::default().fg(Theme::SECONDARY)),
                        ]));
                    }

                    // Bảng chuyển màu xám 16 nấc Grayscale Ramp (#000000 -> #ffffff)
                    lines.push(Line::from(""));
                    lines.push(Line::from(vec![
                        Span::styled("  16-Step Monochrome Grayscale Ramp (000000 ─► ffffff)", Style::default().fg(Theme::PRIMARY).add_modifier(Modifier::BOLD)),
                    ]));
                    lines.push(Line::from(vec![
                        Span::styled("  ─────────────────────────────────────────────────────────────────────────────", Style::default().fg(Theme::NEUTRAL_100)),
                    ]));
                    lines.push(Line::from(""));

                    let mut ramp_spans = vec![Span::raw("    ")];
                    for &c in &Theme::GRAYSCALE_16 {
                        ramp_spans.push(Span::styled("████", Style::default().fg(c)));
                    }
                    lines.push(Line::from(ramp_spans));
                    lines.push(Line::from(""));

                    let hex_steps = [
                        ("000000", Theme::GRAY_00, "Theme::BLACK / Theme::GRAY_00"),
                        ("111111", Theme::GRAY_11, "Theme::GRAY_11"),
                        ("222222", Theme::GRAY_22, "Theme::GRAY_22 (SURFACE_ELEVATED)"),
                        ("333333", Theme::GRAY_33, "Theme::GRAY_33"),
                        ("444444", Theme::GRAY_44, "Theme::GRAY_44 (NEUTRAL_200)"),
                        ("555555", Theme::GRAY_55, "Theme::GRAY_55 (MUTED)"),
                        ("666666", Theme::GRAY_66, "Theme::GRAY_66 (NEUTRAL_300)"),
                        ("777777", Theme::GRAY_77, "Theme::GRAY_77"),
                        ("888888", Theme::GRAY_88, "Theme::GRAY_88 (SECONDARY / NEUTRAL_400)"),
                        ("999999", Theme::GRAY_99, "Theme::GRAY_99"),
                        ("aaaaaa", Theme::GRAY_AA, "Theme::GRAY_AA"),
                        ("bbbbbb", Theme::GRAY_BB, "Theme::GRAY_BB"),
                        ("cccccc", Theme::GRAY_CC, "Theme::GRAY_CC"),
                        ("dddddd", Theme::GRAY_DD, "Theme::GRAY_DD"),
                        ("eeeeee", Theme::GRAY_EE, "Theme::GRAY_EE"),
                        ("ffffff", Theme::GRAY_FF, "Theme::WHITE / Theme::GRAY_FF (PRIMARY)"),
                    ];

                    for (hex_code, color, alias) in hex_steps {
                        lines.push(Line::from(vec![
                            Span::raw("    "),
                            Span::styled("■ ", Style::default().fg(color)),
                            Span::styled(format!("#{:<8} ", hex_code), Style::default().fg(color)),
                            Span::styled(alias, Style::default().fg(Theme::SECONDARY)),
                        ]));
                    }

                    tabs.render_container(main_chunks[1], lines, f);
                }
            }

            // 3. STATUS BAR ĐÁY (Theo đúng chuẩn DESIGN.md Mục 5: Clean, Minimal, Tinh gọn)
            let mut status_bar = StatusBarWidget::new();
            status_bar.add_left(Span::styled("main", Style::default().fg(Theme::PRIMARY).add_modifier(Modifier::BOLD)));
            status_bar.add_left(Span::styled("3 files changed", Style::default().fg(Theme::SECONDARY)));
            status_bar.add_left(Span::styled("✔ Ready", Style::default().fg(Theme::SUCCESS)));

            status_bar.add_right(Span::styled(format!("Tab [{}/5]", tabs.selected() + 1), Style::default().fg(Theme::ACCENT)));
            status_bar.add_right(Span::styled("Esc: Exit", Style::default().fg(Theme::MUTED)));

            status_bar.render(main_chunks[2], f);
        })?;

        // Polling sự kiện phím và chuột
        if event::poll(Duration::from_millis(30))? {
            match event::read()? {
                Event::Key(key) => {
                    if key.kind == KeyEventKind::Press {
                        let is_ctrl_c = key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c');
                        if is_ctrl_c {
                            break;
                        }

                        // 1. Chuyển Tab riêng biệt qua phím số 1..5 hoặc Tab / Shift+Tab (không chiếm phím Left/Right)
                        let mut target_tab = None;
                        match key.code {
                            KeyCode::Char('1') => target_tab = Some(0),
                            KeyCode::Char('2') => target_tab = Some(1),
                            KeyCode::Char('3') => target_tab = Some(2),
                            KeyCode::Char('4') => target_tab = Some(3),
                            KeyCode::Char('5') => target_tab = Some(4),
                            KeyCode::Tab => {
                                tabs.select_next();
                                if tabs.selected() == 3 {
                                    tabs.set_auto_scroll(true); // Tab 4 (Logs): Tiếp tục sticky follow
                                } else {
                                    tabs.set_auto_scroll(false); // Tab 1, 2, 3, 5: Khởi đầu từ đầu trang
                                    tabs.scroll_to_top();
                                }
                                continue;
                            }
                            KeyCode::BackTab => {
                                tabs.select_prev();
                                if tabs.selected() == 3 {
                                    tabs.set_auto_scroll(true); // Tab 4 (Logs): Tiếp tục sticky follow
                                } else {
                                    tabs.set_auto_scroll(false); // Tab 1, 2, 3, 5: Khởi đầu từ đầu trang
                                    tabs.scroll_to_top();
                                }
                                continue;
                            }
                            _ => {}
                        };

                        if let Some(tab_idx) = target_tab {
                            tabs.set_selected(tab_idx);
                            if tab_idx == 3 {
                                tabs.set_auto_scroll(true); // Tab 4 (Logs): Bật sticky follow
                            } else {
                                tabs.set_auto_scroll(false); // Tab 1, 2, 3, 5: Khởi đầu từ đầu trang
                                tabs.scroll_to_top();
                            }
                            continue;
                        }

                        // 2. Tab 1 (Form): FormManager xử lý trước (Widget-First Event Delegation)
                        // Khi popup dropdown/overlay đang mở, Esc/Enter sẽ được tiêu thụ (Consumed) để đóng/lưu popup
                        // và KHÔNG làm thoát chương trình TUI!
                        if tabs.selected() == 0 {
                            if form.handle_event(key) == EventResult::Consumed {
                                continue;
                            }
                        }

                        // 3. Thoát chương trình khi nhấn Esc hoặc q (chỉ khi không có overlay nào tiêu thụ)
                        if key.code == KeyCode::Char('q') || key.code == KeyCode::Esc {
                            break;
                        }

                        // 4. Các Tab còn lại: TabsWidget xử lý cuộn nội dung
                        if tabs.selected() != 0 {
                            tabs.handle_event(key);
                        }
                    }
                }
                Event::Mouse(mouse) => {
                    // Lăn chuột cuộn nội dung container ở bất kỳ Tab nào có thanh cuộn
                    tabs.handle_mouse_event(mouse);
                }
                _ => {}
            }
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;
    println!("=== MASTER SHOWCASE HOÀN TẤT ===");
    Ok(())
}
