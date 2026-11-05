# cli_gui_form

[![License: MIT OR Apache-2.0](https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE-MIT)
[![Rust](https://img.shields.io/badge/Rust-2024%20Edition-orange.svg)](https://www.rust-lang.org)
[![Ratatui](https://img.shields.io/badge/Ratatui-0.28-green.svg)](https://ratatui.rs)

**cli_gui_form** là thư viện xây dựng giao diện TUI Form và Hệ thống Thiết kế Công nghiệp (Industrial Design System) tối giản, hiệu năng cao dành cho Rust, hoạt động dựa trên [`ratatui`](https://crates.io/crates/ratatui) và [`crossterm`](https://crates.io/crates/crossterm).

Được thiết kế lấy cảm hứng từ phong cách tối giản của **Linear** và **Vercel**: Loại bỏ hoàn toàn sự rườm rà (zero-noise), tối ưu trải nghiệm bàn phím mượt mà, zero-allocation trong vòng lặp render, và có cơ chế bảo vệ chống vỡ khung viền terminal.

---

## ✨ Tính Năng Nổi Bật

* 🎯 **Quản lý Vòng đời Focus & Viewport (`FormManager`)**:
  * Điều hướng bàn phím toàn diện (`Tab` / `Shift+Tab`, mũi tên `Up` / `Down`).
  * Tự động tính toán Viewport và cuộn thông minh khi danh sách control vượt quá chiều cao màn hình terminal.
  * Hỗ trợ định vị con trỏ nhấp nháy native (`frame.set_cursor_position`).
* 🧩 **Bộ Widget Đầy Đủ & Tinh Gọn**:
  * `InputWidget`: Ô nhập văn bản dạng Text và Password với con trỏ inline, văn bản gợi ý mờ (`placeholder`) khi ô rỗng theo chuẩn `Theme::NEUTRAL_300` và thao tác chỉnh sửa tức thì.
  * `CheckboxWidget`: Hộp kiểm logic bật/tắt (`[✔]` / `[ ]`).
  * `RadioWidget`: Nhóm lựa chọn một giá trị duy nhất, điều hướng ngang độc lập (`Left`/`Right` thay đổi tùy chọn, `Up`/`Down` chuyển ô form mà không làm nhảy giá trị).
  * `SelectWidget`: **Menu thả xuống dạng Dropdown Popup Overlay (`<select>` / `<option>`)**, chiếm 3 dòng gọn gàng khi đóng, bấm `Enter`/`Space` bung menu nổi đè lên trên với cơ chế `Clear` layer và viền bo tròn, hỗ trợ cuộn `Up`/`Down`, `Enter` để chọn, `Esc` để đóng.
  * `ListWidget`: **Menu danh sách điều hướng cây phân cấp (Hierarchical Sub-items Tree Navigation)**, hỗ trợ đi sâu vào danh mục con bằng phím `Right` / `Enter`, quay lại danh mục cha bằng phím `Left` / `Esc`, và chọn giá trị mục lá.
  * `ButtonWidget`: Nút hành động với hiệu ứng đảo màu khi focus và cơ chế phát tín hiệu submit.
  * `CardWidget`: Khung Panel / Card nhúng tiêu đề trực tiếp lên nắp viền trên, hỗ trợ cả viền vuông (`┌─┐`) lẫn viền bo tròn (`╭─╮`).
  * `TaskWidget`: Quản lý tác vụ nền đa pha (Multi-phase Task Lifecycle), chuyển đổi mượt mà từ Spinner bất định (Pulse, Braille Dots) sang thanh đo tiến trình có đơn vị và thời gian thực tế.
  * `ShimmerWidget`: Dải sáng quét động (Gradient shimmer) 60 FPS mô phỏng tiến trình đồng bộ ngầm.
  * `TabsWidget`: Hệ thống tab nối liền thân panel hoàn chỉnh (`╭─┬─╮`, `╰`, `╯`, `┴`), tích hợp thanh cuộn `█`, hỗ trợ trọn vẹn **con lăn chuột** (`ScrollUp` / `ScrollDown`) và phím cuộn trang (`PageUp`/`PageDown`/`Home`/`End`). Có chế độ **Sticky Follow** tự động bám đáy cho Live Logs.
  * `StatusBarWidget`: Thanh trạng thái đáy tinh tế chia 2 phân vùng trái - phải.
* 🎨 **Hệ thống Biểu tượng Zero-Allocation (`Icons`)**:
  * 22 biểu tượng Unicode được chuẩn hóa hiển thị 2 cell (1 glyph + 1 space đệm) giúp thẳng hàng tuyệt đối.
  * Cơ chế `Icons::span()` và `Icons::line()` tự động triệt tiêu cờ `Modifier::BOLD` riêng cho glyph icon, loại bỏ hoàn toàn hiện tượng co rút 1 cell trên Windows Terminal.
* 🌈 **Hệ màu Đa vai trò (`Theme`)**:
  * Semantic Roles cơ bản (`Primary`, `Secondary`, `Accent`, `Success`, `Warning`, `Error`, `Muted`).
  * Extended Palette chuyên sâu: `CYAN` (Network/Latency), `PURPLE` (AI Engine), `MAGENTA` (Auth/Tokens), `ORANGE` (Queues/Pipelines), `INDIGO` (Git/Branches), `EMERALD` (Uptime/DBs), `SKY` (Cloud Infra), `CRITICAL` (Panics/Alerts).
* 🛡️ **Bảo vệ Cạnh Viền (Border Overflow Protection)**:
  * Tự động cắt tỉa văn bản tràn biên, ngăn ngừa triệt để tình trạng vỡ hoặc lệch viền mép phải.
* 📜 **Giấy phép Kép**: MIT hoặc Apache-2.0, linh hoạt cho dự án thương mại lẫn mã nguồn mở.

---

## 📦 Cài Đặt

Thêm `cli_gui_form` và `ratatui` vào tệp `Cargo.toml`:

```toml
[dependencies]
cli_gui_form = "0.1.0"
crossterm = "0.28"
ratatui = "0.28"
```

Hoặc thêm nhanh qua dòng lệnh `cargo`:

```bash
cargo add cli_gui_form crossterm ratatui
```

---

## 🚀 Khởi Động Nhanh (Quick Start)

Dưới đây là một ví dụ tối giản hoàn chỉnh về việc khởi tạo form tương tác nhập liệu:

```rust,no_run
use cli_gui_form::{
    ButtonWidget, CheckboxWidget, EventResult, FormManager, FormValue, InputMode,
    InputWidget, ListWidget, RadioWidget,
};
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io::{self, stdout};

fn main() -> io::Result<()> {
    // 1. Cấu hình terminal ở raw mode
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout))?;

    // 2. Khởi tạo FormManager và đăng ký các widget
    let mut form = FormManager::new();
    form.add_widget(Box::new(
        InputWidget::new("user", "Tên đăng nhập", InputMode::Text)
            .with_placeholder("admin@company.com"),
    ));
    form.add_widget(Box::new(
        InputWidget::new("pass", "Mật khẩu", InputMode::Password)
            .with_placeholder("••••••••••••"),
    ));
    form.add_widget(Box::new(CheckboxWidget::new("remember", "Ghi nhớ phiên đăng nhập", true)));
    
    // RadioWidget: Left/Right chọn nhanh, Up/Down chuyển ô
    form.add_widget(Box::new(RadioWidget::new(
        "env",
        "Môi trường",
        vec!["Phát triển (Dev)".into(), "Thử nghiệm (Staging)".into(), "Production".into()],
    )));

    // ListWidget: Menu có cây phân cấp
    let mut deploy_target = ListWidget::new("target", "Mục tiêu triển khai", vec![
        "Docker Container Local".into(),
        "Kubernetes Cluster".into(),
        "Serverless Edge Network".into(),
    ]);
    deploy_target.add_sub_item(1, "Cluster US-East");
    deploy_target.add_sub_item(1, "Cluster AP-Southeast (Singapore)");
    deploy_target.add_sub_item(2, "Cloudflare Workers");
    deploy_target.add_sub_item(2, "Vercel Edge Functions");
    form.add_widget(Box::new(deploy_target));

    form.add_widget(Box::new(ButtonWidget::new_primary("submit", "XÁC NHẬN TRIỂN KHAI")));

    // 3. Vòng lặp sự kiện
    loop {
        terminal.draw(|f| {
            form.render(f.area(), f);
        })?;

        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                if key.code == KeyCode::Esc {
                    break;
                }
                if form.handle_event(key) == EventResult::Submitted {
                    let values = form.get_values();
                    // Nhận toàn bộ giá trị form đã nhập tại đây
                    break;
                }
            }
        }
    }

    // 4. Khôi phục terminal nguyên trạng
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}
```

---

## 📚 Hướng Dẫn Sử Dụng Các Widget Chính

### 1. `ListWidget` (Menu Phân Cấp & Điều Hướng Cây Con)

`ListWidget` được thiết kế đặc thù cho các danh sách tác vụ hoặc cấu hình hệ thống đòi hỏi điều hướng dạng cây:

* **Phím `Up` / `Down`**: Di chuyển con trỏ giữa các mục cùng cấp.
* **Phím `Right` (hoặc `Enter` vào mục cha)**: Mở rộng và đi vào danh mục con (Sub-items).
* **Phím `Left` (hoặc `Esc`)**: Thu gọn và quay lại danh mục cha.
* **Phím `Enter` tại mục lá**: Chọn mục đó và gửi tín hiệu chọn (`FormValue::Select(index, label)`).

```rust
use cli_gui_form::{ListWidget, FormWidget};

// Khởi tạo menu cấp 1
let mut list = ListWidget::new("deploy_menu", "Chọn Target Triển Khai", vec![
    "1. Production Deploy".into(),
    "2. Preview Environments".into(),
    "3. Local Staging".into(),
]);

// Bổ sung các mục con phân cấp (Sub-items) cho mục index 0 ("1. Production Deploy")
list.add_sub_item(0, "AWS us-east-1 (N. Virginia)");
list.add_sub_item(0, "GCP asia-southeast1 (Singapore)");
list.add_sub_item(0, "Bare-metal On-premise");

// Bổ sung các mục con cho mục index 1 ("2. Preview Environments")
list.add_sub_item(1, "PR-104 Feature Flag Preview");
list.add_sub_item(1, "Staging Delta Snapshot");

// Trong FormManager hoặc standalone:
// - Nhấn Phím Phải (→) tại mục "1. Production Deploy" để mở nhánh con
// - Nhấn Phím Trái (←) để thoát ra menu cha
```

### 2. `RadioWidget` (Nhóm Radio Chọn Ngang Tinh Gọn)

Thiết kế tách bạch luồng bàn phím giúp form không bao giờ bị nhảy trạng thái ngoài ý muốn:
* `Left` / `Right`: Lựa chọn qua lại giữa các tùy chọn radio trong cùng một hàng.
* `Up` / `Down`: Chuyển focus sang ô form khác phía trên/dưới mà **không làm thay đổi** lựa chọn hiện tại.

```rust
use cli_gui_form::RadioWidget;

let radio = RadioWidget::new(
    "auth_method",
    "Phương thức xác thực",
    vec!["API Token".into(), "OAuth2".into(), "SSH Key".into()],
).with_selected(0);
```

### 3. `SelectWidget` (Dropdown Popup Overlay ─ HTML `<select>` / `<option>`)

Điều khiển chọn danh sách thả xuống mô phỏng trực quan phần tử `<select>` của HTML:

* **Khi đóng**: Chiếm 3 dòng cố định như ô input (`[ Selected Value... ▾ ]`), không làm lệch bố cục Form. Phím `Up` / `Down` được nhả cho `FormManager` để di chuyển focus lên/xuống các ô khác.
* **Khi mở**: Nhấn `Enter` hoặc `Space` để bung menu popup nổi (overlay) đè lên các widget phía dưới với cơ chế `Clear` layer chống xuyên thấu.
* **Điều hướng trong popup**:
  * `Up` / `Down` (hoặc `k` / `j`): Di chuyển con trỏ highlight giữa các options.
  * `Enter` / `Space`: Chọn option đang highlight và tự động đóng dropdown.
  * `Esc`: Đóng dropdown mà không thay đổi lựa chọn.
  * Tự động lật ngược lên trên nếu vị trí ô select nằm sát mép đáy màn hình terminal.

```rust
use cli_gui_form::{SelectWidget, FormWidget};

// Khởi tạo SelectWidget với danh sách các option (hỗ trợ cả cặp value / label)
let select = SelectWidget::new(
    "region",
    "Chọn Data Center Lưu Trữ",
    vec![
        ("us-east-1", "US East (N. Virginia)"),
        ("eu-central-1", "Europe (Frankfurt)"),
        ("ap-southeast-1", "Asia Pacific (Singapore)"),
        ("sa-east-1", "South America (São Paulo)"),
    ],
)
.with_placeholder("Vui lòng chọn khu vực... ▾")
.with_selected(0);

// Đăng ký trực tiếp vào FormManager như mọi widget khác:
// form.add_widget(Box::new(select));
```

### 4. `TabsWidget` & `StatusBarWidget` (Hệ Thống Thẻ & Cuộn Chuột)

Hỗ trợ giao diện Dashboard đa tab nối liền với panel container bo góc, tích hợp sẵn thanh cuộn và nhận diện chuột:

```rust
use cli_gui_form::{TabsWidget, StatusBarWidget, Theme};
use crossterm::event::{EnableMouseCapture, DisableMouseCapture, MouseEventKind};
use ratatui::text::{Line, Span};

// 1. Tạo thanh điều hướng Tabs
let mut tabs = TabsWidget::new("nav", vec![
    "1. Overview",
    "2. Table Data",
    "3. Tasks & Spinners",
    "4. Live Logs",
    "5. Settings",
]);

// Bắt sự kiện lăn chuột và phím cuộn
// - tabs.handle_mouse_event(mouse): Cuộn mượt khi lăn chuột
// - tabs.handle_event(key): Cuộn bằng Up/Down/PageUp/PageDown/Home/End
// - tabs.render_container(area, content_lines, frame): Vẽ toàn bộ khung viền bo góc nối liền
```

### 4. `TaskWidget` (Vòng Đời Tác Vụ Đa Pha & Animation)

Hỗ trợ chuyển đổi trạng thái tải bất định sang đo lường tiến trình chính xác:

```rust
use cli_gui_form::{TaskWidget, SpinnerType, Theme};

// 1. Tác vụ đang khởi động (Pulse Spinner 150ms)
let mut task = TaskWidget::new_loading(
    "inference",
    "Khởi động Neural Engine",
    "Đang tải trọng số mô hình...",
    Theme::PURPLE,
).with_spinner_type(SpinnerType::Pulse);

// Nhịp tick animation:
task.tick();

// 2. Chuyển sang thanh tiến trình đo lường phần trăm khi bắt đầu xử lý:
task.switch_to_progress(
    "Đang biên dịch WASM",
    500,      // Tổng số lượng
    "chunks", // Đơn vị
    "14s",    // Thời gian ước tính
    "Đang tối ưu dead code...",
);
task.update_progress(350, "4s");
```

### 5. `CardWidget` & `ShimmerWidget`

```rust
use cli_gui_form::{CardWidget, ShimmerWidget, Icons, Theme};

// Khung Card nhúng tiêu đề lên nắp viền
let card = CardWidget::new("deploy_card", "Trạng thái Hạ tầng")
    .with_rounded(true)
    .with_item("PostgreSQL Cluster", Some(Icons::SUCCESS), "Ready", Theme::SUCCESS)
    .with_item("Redis Cache", Some(Icons::RUN), "Syncing", Theme::CYAN);

// Dải sáng quét động (Gradient Shimmer)
let mut shimmer = ShimmerWidget::new("shimmer", "Đang đồng bộ dữ liệu nền...");
shimmer.tick();
```

---

## 🎨 Bảng Tra Cứu Icon & Màu Sắc

### Biểu Tượng Chuẩn (`Icons`)

| Hằng số | Ký tự | Mã Unicode | Ý nghĩa nghiệp vụ |
| :--- | :---: | :---: | :--- |
| `Icons::SUCCESS` | `✔ ` | `U+2714` | Tác vụ hoàn thành, hợp lệ |
| `Icons::ERROR` | `✗ ` | `U+2716` | Thất bại, lỗi hệ thống |
| `Icons::WARNING` | `⚠ ` | `U+26A0` | Cảnh báo tài nguyên, chú ý |
| `Icons::INFO` | `ℹ ` | `U+2139` | Thông tin hướng dẫn |
| `Icons::RUN` | `▶ ` | `U+25B6` | Đang chạy, đang build (chuẩn 2 cell cân đối) |
| `Icons::BUILD` | `⚙ ` | `U+2699` | Tác vụ cấu hình, biên dịch |
| `Icons::PAUSE` | `⏸ ` | `U+23F8` | Tạm dừng tiến trình |
| `Icons::STOP` | `■ ` | `U+25A0` | Đã dừng hẳn |
| `Icons::CHECKBOX_ON` | `☑ ` | `U+2611` | Đã tích chọn checkbox |
| `Icons::CHECKBOX_OFF`| `☐ ` | `U+2610` | Chưa tích chọn checkbox |
| `Icons::RADIO_ON` | `● ` | `U+25CF` | Đã chọn radio |
| `Icons::RADIO_OFF` | `○ ` | `U+25CB` | Chưa chọn radio |
| `Icons::BRANCH` | `⤷ ` | `U+21B3` | Nhánh rẽ mục con (Sub-item tree) |
| `Icons::POINTER` | `▹ ` | `U+25B8` | Con trỏ chỉ mục đang focus |

### Bảng Màu Hệ Thống (`Theme`)

* **Màu cốt lõi**: `PRIMARY` (`#4589FF`), `SECONDARY` (`#888888`), `SUCCESS` (`#25A249`), `WARNING` (`#F1C21B`), `ERROR` (`#DA1E28`), `ACCENT` (`#0070F3`), `MUTED` (`#555555`).
* **Multi-Role Extended Palette**:
  * `CYAN` (`#50E3C2`): Mạng, độ trễ API, điểm cuối Edge.
  * `PURPLE` (`#7928CA`): AI Engine, Neural Layers, GraphQL.
  * `MAGENTA` (`#F81CE5`): Token bảo mật, Secrets, Webhooks.
  * `ORANGE` (`#FF8800`): Hàng đợi, Background Workers, Pipeline.
  * `INDIGO` (`#5E6AD2`): Nhánh Git, PR Reviews, Linear Tasks.
  * `EMERALD` (`#10B981`): Uptime ổn định, In-memory Caching.
  * `SKY` (`#38BDF8`): Hạ tầng đám mây, Docker, Kubernetes.
  * `CRITICAL` (`#FF0055`): Lỗi nghiêm trọng, Panic cấp bách.

---

## 🧪 Chạy Thử Nghiệm Các Ví Dụ Mẫu (Examples)

Thư viện tích hợp sẵn bộ ví dụ trực quan phong phú:

```bash
# 🔥 Master Showcase Toàn Diện: 5 Tab, Form đầy đủ, Bảng biểu, Animation, Icon & Bảng màu
cargo run --example demo_master

# Thử nghiệm FormManager tương tác nhập liệu
cargo run --example demo

# Thử nghiệm Hệ thống Tabs điều hướng & Status Bar
cargo run --example demo_tabs

# Thử nghiệm Bảng palette hiển thị trọn bộ 22 Icon Unicode
cargo run --example demo_icons_palette

# Thử nghiệm vòng đời Spinner Pulse và Task Progress
cargo run --example demo_pulse_task

# Thử nghiệm Shimmer gradient bar 60 FPS
cargo run --example status_shimmer

# Thử nghiệm Bảng dữ liệu viền phẳng tối giản
cargo run --example demo_table
```

---

## 🏛️ Triết Lý Kiến Trúc

1. **YAGNI & Ponytail Style**: Tối giản, thực dụng, tập trung vào hiệu năng cao và an toàn bộ nhớ (Ownership/Lifetime) trong Rust.
2. **Khắc phục lỗi hiển thị Terminal**: Tự động giải quyết các lỗi cố hữu của terminal như co glyph khi in đậm, lệch viền khi chuỗi ký tự quá dài, mất con trỏ nhấp nháy.
3. **Thread Safety**: Mọi widget đều implement `Send`, hỗ trợ cập nhật dữ liệu mượt mà từ các background thread thông qua các cấu trúc an toàn như `Arc<Mutex<T>>` mà không gây nghẽn UI Thread.

---

## 📄 Giấy Phép (License)

Dự án được cấp phép kép theo:

* **MIT License** ([LICENSE-MIT](LICENSE-MIT) hoặc [http://opensource.org/licenses/MIT](http://opensource.org/licenses/MIT))
* **Apache License, Version 2.0** ([LICENSE-APACHE](LICENSE-APACHE) hoặc [http://www.apache.org/licenses/LICENSE-2.0](http://www.apache.org/licenses/LICENSE-2.0))
