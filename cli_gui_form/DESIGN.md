# Minimal — TUI Design System

> Clean, focused, zero noise. Inspired by Vercel and Linear's terminal aesthetics.

---

## 📌 Implementation Checklist & Component Status

| Phân hệ / Component | Module thực thi | Trạng thái | Ghi chú kiến trúc |
| :--- | :--- | :---: | :--- |
| **Color Palette & Theme** | `src/theme.rs` | `[x]` Hoàn thành | Semantic roles, Neutral scale, Multi-role Extended Palette |
| **Typography & Hierarchy** | `src/theme.rs` | `[x]` Hoàn thành | BOLD + Primary, Dim/Muted fallback, No italic |
| **Borders & Box Drawing** | `card.rs`, `tabs.rs` | `[x]` Hoàn thành | Bo góc (`╭╮╯╰`), nối chữ T (`┬┴├┤`), nắp gập liền panel |
| **Buttons / Actions** | `src/widgets/button.rs` | `[x]` Hoàn thành | Focused invert/reverse, tiền tố `▸`, phím Enter submit |
| **Input Fields** | `src/widgets/input.rs` | `[x]` Hoàn thành | Text/Password, con trỏ nhấp nháy inline, viền focus `#0070f3` |
| **Checkbox Widget** | `src/widgets/checkbox.rs`| `[x]` Hoàn thành | Toggle logic `[✔]` / `[ ]`, phím Space/Enter |
| **Radio Groups** | `src/widgets/radio.rs` | `[x]` Hoàn thành | `Left`/`Right` chọn nhanh, `Up`/`Down` chuyển ô form an toàn |
| **Select Dropdown** | `src/widgets/select.rs` | `[x]` Hoàn thành | Floating popup overlay (`<select>`/`<option>`), Clear pass, tự lật hướng |
| **Lists / Menus** | `src/widgets/list.rs` | `[x]` Hoàn thành | Điều hướng cây phân cấp: `→` vào mục con, `←` thoát ra, `Enter` chọn |
| **Panels / Cards** | `src/widgets/card.rs` | `[x]` Hoàn thành | Tiêu đề nhúng nắp trên, tùy chọn viền vuông / viền bo tròn |
| **Tables** | `demo_master.rs` | `[x]` Hoàn thành | Header tách biệt gạch ngang `─`, cột canh lề tỉ mỉ, không viền ngoài |
| **Tabs & Container** | `src/widgets/tabs.rs` | `[x]` Hoàn thành | Nắp bo góc mở thông panel, lăn chuột & bàn phím cuộn, sticky log |
| **Status Bar** | `src/widgets/status_bar.rs` | `[x]` Hoàn thành | Thanh đáy chia 2 cụm trái - phải, ngăn cách ` ─ ` |
| **Icons & Indicators** | `src/icons.rs` | `[x]` Hoàn thành | 22 glyph Unicode 2-cell, chuẩn hóa `▶ `, tự động lọc BOLD |
| **Spinners & Animation** | `task.rs`, `shimmer.rs` | `[x]` Hoàn thành | Pulse 150ms, Braille dots 80ms, Shimmer gradient 60 FPS |

---

## 1. Theme Overview

- **Mood**: Minimal, professional, calm
- **Density**: Balanced — generous whitespace without wasting terminal real estate
- **Target**: Developer tools, CLI utilities, AI agent interfaces
- **Terminal**: 256-color minimum, TrueColor recommended

---

## 2. Color Palette `[x]`

### Semantic Roles

| Role | Hex | ANSI 256 | ANSI 16 | Usage |
|------|-----|----------|---------|-------|
| Background | `#0a0a0a` | `232` | `black` | Main background |
| Foreground | `#ededed` | `255` | `white` | Default text |
| Primary | `#ffffff` | `15` | `bright white` | Key actions, focus states |
| Secondary | `#888888` | `245` | `bright black` | Supporting text |
| Accent | `#0070f3` | `33` | `blue` | Links, highlights |
| Success | `#00c853` | `41` | `green` | Positive status |
| Warning | `#f5a623` | `214` | `yellow` | Caution status |
| Error | `#ee0000` | `196` | `red` | Error status |
| Muted | `#555555` | `240` | `bright black` | Disabled, hints |
| Surface | `#1a1a1a` | `234` | `black` | Panels, cards |

### Multi-Role Extended Palette (Đã triển khai trong `Theme`) `[x]`

| Constant | Hex | RGB | Phân bổ nghiệp vụ chuyên sâu |
| :--- | :--- | :--- | :--- |
| `Theme::CYAN` | `#50E3C2` | `(80, 227, 194)` | Network, API Latency, Endpoints, Edge Nodes |
| `Theme::PURPLE` | `#7928CA` | `(121, 40, 202)` | AI Engine, Neural Layers, GraphQL, Transformers |
| `Theme::MAGENTA` | `#F81CE5` | `(248, 28, 229)` | Auth Tokens, Webhooks, Security Keys, Secrets |
| `Theme::ORANGE` | `#FF8800` | `(255, 136, 0)` | Queues, Background Workers, Build Pipelines |
| `Theme::INDIGO` | `#5E6AD2` | `(94, 106, 210)` | Branches, PR Reviews, Linear Task References |
| `Theme::EMERALD` | `#10B981` | `(16, 185, 129)` | Healthy Uptime, In-Memory DBs, Memory Safe |
| `Theme::SKY` | `#38BDF8` | `(56, 189, 248)` | Cloud Infra, Docker Containers, Kubernetes Pods |
| `Theme::CRITICAL` | `#FF0055` | `(255, 0, 85)` | Fatal Panics, Kernel Faults, Immediate Alerts |
| `Theme::SURFACE_ELEVATED` | `#222222` | `(34, 34, 34)` | Background cho card và popover |
| `Theme::BORDER_FOCUS` | `#0070F3` | `(0, 112, 243)` | Viền khi ô form được kích hoạt con trỏ |

### Neutral Scale `[x]`

| Step | Hex | Usage |
|------|-----|-------|
| 50 | `#1a1a1a` | Subtle backgrounds, surface |
| 100 | `#2a2a2a` | Borders, dividers |
| 200 | `#444444` | Disabled text |
| 300 | `#666666` | Placeholder text |
| 400 | `#888888` | Secondary text |
| 500 | `#ededed` | Body text |

---

## 3. Typography & ASCII Art `[x]`

- **Header font**: `small` (figlet) — compact, not flashy
- **Body text**: plain terminal font
- **Emphasis**: `bold` only — avoid italic in terminals (poor support)
- **Code/values**: `dim` background or Accent color

### Text Hierarchy

| Level | Style | Example Usage |
|-------|-------|---------------|
| H1 | BOLD + Primary (`#ffffff`) | App title, Header |
| H2 | BOLD + Foreground (`#ededed`) | Section headers |
| H3 | BOLD + Secondary (`#888888`) | Subsection headers |
| Body | Foreground (`#ededed`) | Content text |
| Caption | Muted (`#555555`) | Help text, timestamps |
| Label | BOLD + Secondary | Form labels |

---

## 4. Borders & Box Drawing `[x]`

### Primary Border

```
┌──────────────┐
│   content    │
└──────────────┘
```

Single-line box drawing. Clean and lightweight.

### Parts Table

| Part | Character | Usage |
|------|-----------|-------|
| top_left | `┌` / `╭` | Panel corners (vuông hoặc bo tròn) |
| top_right | `┐` / `╮` | |
| bottom_left | `└` / `╰` | |
| bottom_right | `┘` / `╯` | |
| horizontal | `─` | Horizontal lines |
| vertical | `│` | Vertical lines |
| cross | `┼` | Table intersections |
| tee_down | `┬` | Table header separator |
| tee_up | `┴` | Table footer |
| tee_right | `├` | Left junction |
| tee_left | `┤` | Right junction |

---

## 5. Components `[x]`

### Buttons / Actions `[x]` (`ButtonWidget`)

```
 ▶ Submit    Cancel    Help
   ↑          ↑        ↑
 focused   unfocused  muted
```

- Focused: `reverse` (white bg, black fg) hoặc Solid Accent
- Unfocused: plain Foreground text / Dark background
- Disabled: Muted + dim
- Không sử dụng `Modifier::BOLD` khi hover/focus để bảo vệ 100% hình dạng glyph Unicode (`▶`).

#### Bảng Cụm Màu Nút Bấm An Toàn (Safe Button Color Presets) `[x]`

> **Ranh giới bảo vệ thị giác (Guardrail):** Tuyệt đối **KHÔNG dùng nền xám trung gian** (`Theme::GRAY_22` `#222222` hoặc `Theme::GRAY_33` `#333333`) làm background cho glyph `▶` vì bộ dựng hình Windows Terminal sẽ bị lỗi gamma/subpixel antialiasing làm co dẹt icon.
> Luôn sử dụng các cặp màu có **độ tương phản cực đại** dưới đây:

| Mã Preset | Phong cách | Màu Blur (Nghỉ) | Màu Focus (Hover) | Ngữ cảnh sử dụng |
| :--- | :--- | :--- | :--- | :--- |
| **`INVERT_APPLE`** | Invert High-Contrast | `Theme::BG` / `Theme::SECONDARY` | `Theme::PRIMARY` / `Theme::BG` | Nút hành động chính chuẩn Linear/Apple (Nền trắng, chữ đen). |
| **`SOLID_VERCEL`** | Solid Vercel Blue | `Theme::ACCENT` / `Theme::WHITE` | `Theme::PRIMARY` / `Theme::ACCENT` | Nút triển khai, tạo mới (Primary CTA). |
| **`AI_VIOLET`** | Neural Violet | `Theme::PURPLE` / `Theme::WHITE` | `Theme::PRIMARY` / `Theme::PURPLE` | Tính năng AI Agent, Dubbing, Podcast Generator. |
| **`EMERALD_SUCCESS`**| Emerald Green | `Theme::BG` / `Theme::EMERALD` | `Theme::EMERALD` / `Theme::BLACK` | Nút hoàn tất, xác nhận lưu, xuất bản file. |
| **`MINIMAL_GHOST`** | Subtle Ghost | `Theme::BG` / `Theme::SECONDARY` | `Theme::BG` / `Theme::ACCENT` | Nút phụ (Secondary/Cancel/Help) phong cách GitHub CLI. |
| **`AMBER_WARN`** | Amber Pipeline | `Theme::BG` / `Theme::ORANGE` | `Theme::ORANGE` / `Theme::BLACK` | Nút cảnh báo, reset cache, hủy tiến trình. |

#### ⚠️ Chú ý Kỹ thuật về TrueColor RGB / Hex và Antialiasing trên Terminal

1. **TrueColor 24-bit (`Color::Rgb(r, g, b)`):** Hệ thống hỗ trợ 100% không gian màu TrueColor (16 triệu màu / Hex `#xxxxxx`), không bị gò bó trong 16 màu ANSI cơ bản. Các mã màu có sắc tố bão hòa (như Vercel Blue `#0070f3`, Linear Violet `#7928ca`, Emerald `#10b981`, Amber `#ff8800`) luôn đảm bảo glyph Unicode hiển thị sắc nét và chuẩn xác 100%.
2. **Hiện tượng Mid-tone Gray Clipping:**
   - Khi dùng màu nền là các sắc độ xám trung gian (Mid-tone Grays như `#222222`, `#333333`) đi cùng chữ trắng sáng (`#ffffff`, `#ededed`), bộ khử răng cưa của Windows Terminal (DirectWrite / ClearType) bị xung đột thuật toán Gamma Blend.
   - Các pixel biên ở chóp nhọn của glyph `▶` bị hòa lẫn vào nền xám tối, làm mất nét hoặc gây cảm giác icon bị co rút dẹt lại.
3. **2 Nguyên tắc Vàng khi chọn bất kỳ mã màu Hex / RGB tùy biến:**
   - **Nút có nền khối (Solid Button):** Dùng bất kỳ màu RGB nổi bật nào làm nền (`#0070f3`, `#7928ca`, `#10b981`, `#e91e63`, `#ff5722`...) đi cùng chữ `Theme::WHITE` hoặc `Theme::BLACK`. Icon luôn to tròn 100%.
   - **Nút không nền (Ghost/Outline):** Nền giữ nguyên nền tối sâu `Theme::BG` (`#0a0a0a`), chữ dùng màu RGB có sắc tố (Accent Blue, Cyan, Amber, Emerald...). Tránh dùng chữ trắng tinh `#ffffff` đơn độc trên nền đen.

### Input Fields `[x]` (`InputWidget`)

```
  Email: │user@example.com        │
         └────────────────────────┘
```

- Active: viền `Theme::BORDER_FOCUS` (`#0070f3`), con trỏ native hiển thị nhấp nháy
- Inactive: viền `Theme::MUTED` (`#555555`)
- Placeholder: văn bản gợi ý mờ bằng `Theme::NEUTRAL_300` (`#666666`) khi ô chưa có dữ liệu
- Password mode: tự động mã hóa ký tự dạng `••••••••` khi người dùng nhập dữ liệu

### Select Dropdown `[x]` (`SelectWidget`)

```
  Region: │ US East (N. Virginia)      ▾ │
          ├──────────────────────────────┤
          │ ▸ US East (N. Virginia)    ✔ │
          │   Europe (Frankfurt)         │
          │   Asia Pacific (Singapore)   │
          ╰──────────────────────────────╯
```

- **Đóng**: Khung 3 dòng gọn gàng, hiển thị nhãn và mũi tên `▾`. Phím `Up`/`Down` nhả cho `FormManager` chuyển ô form.
- **Mở (`Enter`/`Space`)**: Bung menu nổi floating popup overlay đè lên trên các widget bên dưới, dùng `Clear` widget quét sạch layer đáy.
- **Điều hướng trong popup**:
  - `Up`/`Down` (hoặc `k`/`j`): Duyệt highlight giữa các options.
  - `Enter`/`Space`: Chọn option hiện tại và đóng menu.
  - `Esc`: Đóng menu mà không đổi lựa chọn.
  - Tự động lật hướng lên trên nếu không đủ không gian phía dưới màn hình.

### Tables `[x]` (`demo_master.rs` / `demo_table.rs`)

```
  NAME              STATUS         BRANCH     COMMIT    LATENCY    TIME
  ─────────────────────────────────────────────────────────────────────────────
  deploy-api        ✔ Ready        main       7f8a91c   12ms       2m ago
  worker-engine     ▶ Building     staging    3c4d5e1   45ms       just now
```

- Không viền bao ngoài, header phân cách bằng đường kẻ mờ `─` (`Theme::NEUTRAL_100`).
- Cột số liệu và thời gian canh lề rõ ràng.

### Lists / Menus `[x]` (`ListWidget`)

Hỗ trợ điều hướng danh mục phẳng và cây phân cấp (Sub-items):

```
  Root Menu:
    General Settings
  ▸ Deployments
    ↳ Production (AWS us-east-1)
    ↳ Preview (Vercel Edge)
    ↳ Staging (Docker Local)
    Integrations
```

- **Điều hướng phím**:
  - `Up` / `Down`: Di chuyển con trỏ giữa các mục cùng cấp.
  - `Right` (hoặc `Enter` vào mục cha có mục con): Mở rộng và đi vào danh mục con.
  - `Left` (hoặc `Esc`): Thoát khỏi danh mục con, quay về danh mục cha.
  - `Enter` tại mục lá: Kích hoạt lựa chọn (`FormValue::Select(idx, text)`).
- **Trực quan hóa**:
  - Mục được chọn: tiền tố `▸ ` + `Modifier::BOLD` + `Theme::PRIMARY`.
  - Mục con: thụt lề cấp 2 với tiền tố nhánh rẽ `↳ ` (`Icons::BRANCH`).
  - Mục cha có thể mở rộng: biểu tượng `...` ở mép phải.

### Panels / Cards `[x]` (`CardWidget`)

```
┌─ Deploy Status ──────────────┐
│                              │
│  Production    ✔ Ready       │
│  Preview       ▶ Building    │
│  Staging       ✔ Ready       │
│                              │
└──────────────────────────────┘
```

- Tiêu đề nhúng trực tiếp vào nắp viền trên.
- Hỗ trợ cả viền vuông (`┌─┐`) lẫn viền bo tròn (`╭─╮`).
- Hiển thị danh sách trạng thái kèm icon và nhãn màu.

### Tabs & Container `[x]` (`TabsWidget`)

```
╭──────────┬──────┬──────────┬─────────────╮
│ Overview │ Logs │ Settings │ Deployments │
├──────────╯      ╰──────────┴─────────────┴───────────────╮
```

- **Thiết kế**: Nối liền 1-1 giữa tab bar bo góc phía trên và thân panel phía dưới. Active tab mở thông đáy vào nội dung, Inactive tab đóng kín đáy.
- **Cuộn trang & Điều hướng**:
  - Bắt trọn con lăn chuột (`MouseEventKind::ScrollUp` / `ScrollDown`).
  - Phím cuộn: `Up`/`Down`, `PageUp`/`PageDown`, `Home` (lên đầu), `End` (xuống đáy).
  - Tích hợp thanh cuộn tinh tế `█` ở mép phải khi nội dung vượt quá chiều cao viewport.
  - Chế độ **Sticky Follow**: Tự động bám đáy khi có log mới truyền về, tự động tạm dừng khi người dùng chủ động cuộn ngược lên đọc log cũ.
  - Các tab tài liệu/bảng/danh mục khởi đầu an toàn tại dòng đầu tiên (`scroll_offset = 0`), không bị nhảy xuống đáy.

### Status Bar `[x]` (`StatusBarWidget`)

```
 main ─ 3 files changed ─ ✔ Ready                     Tab [1/5] ─ Esc: Exit
```

- Thanh đơn giản ở đáy terminal. Phân chia 2 cụm thông tin bên trái và bên phải, ngăn cách thanh lịch bởi ` ─ `.

---

## 6. Layout & Spacing `[x]`

- **Min terminal width**: `80`
- **Ideal terminal width**: `120`
- **Padding inside panels**: 1 line top/bottom, 1 char left/right
- **Gap between components**: 1 empty line
- **Indent level**: 2 spaces
- **Overflow Guard**: Cơ chế cắt tỉa an toàn bảo vệ cạnh viền phải không bao giờ bị xô lệch khi văn bản bên trong quá dài.

---

## 7. Icons & Indicators `[x]` (`src/icons.rs`)

| Purpose | Constant | Glyph | Unicode | Fallback |
| :--- | :--- | :---: | :---: | :---: |
| Success | `Icons::SUCCESS` | `✓ ` | `U+2713` | `+` |
| Success Heavy | `Icons::SUCCESS_HEAVY` | `✔ ` | `U+2714` | `+` |
| Error / Fail | `Icons::ERROR` | `✗ ` | `U+2716` | `x` |
| Warning | `Icons::WARNING` | `! ` | `U+0021` | `!` |
| Info | `Icons::INFO` | `i ` | `U+0069` | `i` |
| Running / Play | `Icons::RUN` / `PLAY` | `▶ ` | `U+25B6` | `>` |
| Pointer Right | `Icons::POINTER_RIGHT` | `► ` | `U+25BA` | `>` |
| Build / Work | `Icons::BUILD` | `⛯ ` | `U+26EF` | `*` |
| Stop | `Icons::STOP` | `■ ` | `U+25A0` | `[#]` |
| Pause | `Icons::PAUSE` | `॥ ` | `U+0965` | `\|\|` |
| Checkbox on | `Icons::CHECKBOX_ON` | `[✓]` | `U+2713` | `[x]` |
| Checkbox off | `Icons::CHECKBOX_OFF` | `[ ]` | `U+0020` | `[ ]` |
| Radio on | `Icons::RADIO_ON` | `● ` | `U+25CF` | `(•)` |
| Radio off | `Icons::RADIO_OFF` | `○ ` | `U+25CB` | `( )` |
| Sparkle Filled | `Icons::SPARKLE_FILLED` | `✦ ` | `U+2726` | `*` |
| Star Outline | `Icons::STAR_OUTLINE` | `⚝ ` | `U+269D` | `*` |
| Diamond Empty | `Icons::DIAMOND_EMPTY` | `◇ ` | `U+25C7` | `<>` |
| Snowflake | `Icons::SNOWFLAKE` | `❅ ` | `U+2745` | `*` |
| Pointer / Arrow | `Icons::POINTER` | `▹ ` | `U+25B8` | `>` |
| Arrow Right | `Icons::ARROW_RIGHT` | `→ ` | `U+2192` | `->` |
| Branch Sub-level | `Icons::BRANCH` | `⤷ ` | `U+21B3` | `\_` |
| Progress Line | `Icons::PROGRESS_FILLED` / `EMPTY` | `━` / `─` | `U+2501` / `U+2500` | `=` / `-` |
| Progress Parallelogram | `Icons::PROGRESS_PARALLELOGRAM_FILLED` / `EMPTY` | `▰` / `▱` | `U+25B0` / `U+25B1` | `#` / `-` |

#### Safe Geometric Glyphs (Kiểm chứng Windows Terminal & Linux / Zero-Clipping)

| Danh mục | Hằng số Rust | Glyph | Unicode | Fallback ASCII |
| :--- | :--- | :---: | :--- | :--- |
| **Squares** | `Icons::SQUARE_FILLED` | `■ ` | `U+25A0` | `[#]` |
| | `Icons::SQUARE_EMPTY` | `□ ` | `U+25A1` | `[ ]` |
| | `Icons::SQUARE_MEDIUM_FILLED` | `◼ ` | `U+25FC` | `[#]` |
| | `Icons::SQUARE_MEDIUM_EMPTY` | `◻ ` | `U+25FB` | `[ ]` |
| | `Icons::SQUARE_SMALL_FILLED` | `▪ ` | `U+25AA` | `*` |
| | `Icons::SQUARE_DOTTED` / `BOX_DOTTED` | `⬚ ` | `U+2B1A` | `[ ]` |
| | `Icons::SQUARE_SHADOW` / `BOX_SHADOW` | `❒ ` | `U+2752` | `[#]` |
| **Circles / Activity States** | `Icons::CIRCLE_FILLED` / `STATE_ACTIVE` | `● ` | `U+25CF` | `(•)` |
| | `Icons::CIRCLE_EMPTY` / `STATE_INACTIVE` | `○ ` | `U+25CB` | `( )` |
| | `Icons::CIRCLE_DOTTED` / `STATE_IDLE` | `◌ ` | `U+25CC` | `( )` |
| | `Icons::CIRCLE_TARGET` / `RADIO_TARGET` | `◉ ` | `U+25C9` | `(@)` |
| | `Icons::CIRCLE_HALF_LEFT` | `◐ ` | `U+25D0` | `(` |
| | `Icons::CIRCLE_HALF_BOTTOM` | `◒ ` | `U+25D2` | `_` |
| | `Icons::CIRCLE_HALF_RIGHT` | `◑ ` | `U+25D1` | `)` |
| | `Icons::CIRCLE_HALF_TOP` | `◓ ` | `U+25D3` | `^` |
| **Quadrant Arcs** | `Icons::ARC_TOP_LEFT` | `◜ ` | `U+25DC` | `(` |
| | `Icons::ARC_TOP_RIGHT` | `◝ ` | `U+25DD` | `)` |
| | `Icons::ARC_BOTTOM_RIGHT` | `◞ ` | `U+25DE` | `)` |
| | `Icons::ARC_BOTTOM_LEFT` | `◟ ` | `U+25DF` | `(` |
| **Triangles / Pointers** | `Icons::TRIANGLE_UP` | `▲ ` | `U+25B2` | `^` |
| | `Icons::TRIANGLE_DOWN` | `▼ ` | `U+25BC` | `v` |
| | `Icons::TRIANGLE_RIGHT_SMALL` / `POINTER_FILLED` | `▸ ` | `U+25B8` | `>` |
| | `Icons::ARROWHEAD_RIGHT` / `PROMPT_ARROW` | `⮞ ` | `U+2B9E` | `>` |
| **Diamonds** | `Icons::DIAMOND_FILLED` | `◆ ` | `U+25C6` | `<*>` |
| | `Icons::DIAMOND_EMPTY` | `◇ ` | `U+25C7` | `<>` |
| **Media & Audio** | `Icons::MUSIC` | `♪ ` | `U+266A` | `[~]` |
| | `Icons::MUSIC_DOUBLE` | `♫ ` | `U+266B` | `[~~]` |
| **Navigation & System** | `Icons::TAB` | `⇥ ` | `U+21E5` | `->\|` |
| | `Icons::MENU` | `≡ ` | `U+2261` | `==` |
| | `Icons::HOURGLASS` | `⧗ ` | `U+29D7` | `X` |
| | `Icons::SQUARE_CONTAINED` | `▣ ` | `U+25A3` | `[+]` |
| **Technical & Target** | `Icons::PIN` | `⚲ ` | `U+26B2` | `!` |
| | `Icons::SPARK` | `⌁ ` | `U+2301` | `/` |
| | `Icons::BULLSEYE` | `◎ ` | `U+25CE` | `((o))` |
| | `Icons::CROSSHAIR` | `⌖ ` | `U+2316` | `(+)` |
| | `Icons::APPROX` | `≈ ` | `U+2248` | `~` |
| | `Icons::SEARCH` / `FIND` | `⌕ ` | `U+2315` | `(?)` |
| **Directional Arrows** | `Icons::ARROW_UP` | `↑ ` | `U+2191` | `^` |
| | `Icons::ARROW_DOWN` | `↓ ` | `U+2193` | `v` |
| | `Icons::ARROW_LEFT` | `← ` | `U+2190` | `<` |
| | `Icons::ARROW_RIGHT` | `→ ` | `U+2192` | `>` |
| | `Icons::ARROW_UP_DOWN` | `↕ ` | `U+2195` | `^v` |
| | `Icons::ARROW_LEFT_RIGHT` | `↔ ` | `U+2194` | `<>` |
| | `Icons::ARROW_DOUBLE_RIGHT` | `⇒ ` | `U+21D2` | `=>` |
| | `Icons::ARROW_DOUBLE_LEFT` | `⇐ ` | `U+21D0` | `<=` |
| | `Icons::ARROW_DOUBLE_UP` | `⇑ ` | `U+21D1` | `^^` |
| | `Icons::ARROW_DOUBLE_DOWN` | `⇓ ` | `U+21D3` | `vv` |
| | `Icons::ARROW_DOUBLE_BOTH` | `⇔ ` | `U+21D4` | `<=>` |
| | `Icons::ARROW_FAST_RIGHT` | `↠ ` | `U+21A0` | `>>` |
| | `Icons::ARROW_FAST_LEFT` | `↞ ` | `U+219E` | `<<` |
| | `Icons::ARROW_HOOK_RIGHT` | `↪ ` | `U+21AA` | `\_>` |
| | `Icons::ARROW_HOOK_LEFT` | `↩ ` | `U+21A9` | `<_/` |
| | `Icons::ARROW_TO_BAR_LEFT` | `⇤ ` | `U+21E4` | `\|<` |
| | `Icons::ARROW_TO_BAR_RIGHT` | `⇥ ` | `U+21E5` | `>\|` |
| | `Icons::ARROW_UP_RIGHT` / `EXTERNAL_LINK` | `↗ ` | `U+2197` | `/^` |
| | `Icons::ARROW_DOWN_RIGHT` | `↘ ` | `U+2198` | `\v` |
| | `Icons::ENTER` | `↵ ` | `U+21B5` | `<-'` |
| | `Icons::RELOAD` | `↻ ` | `U+21BB` | `@` |
| | `Icons::UNDO` | `↺ ` | `U+21BA` | `@` |
| **Exchange & Harpoons** | `Icons::ARROW_SWAP` / `ARROW_EXCHANGE` | `⇄ ` | `U+21C4` | `<=>` |
| | `Icons::ARROW_TRANSFER` | `⇆ ` | `U+21C6` | `<=>` |
| | `Icons::ARROW_DUPLEX_VERT` | `⇅ ` | `U+21C5` | `^v` |
| | `Icons::ARROW_DUPLEX_VERT_REV` | `⇵ ` | `U+21F5` | `v^` |
| | `Icons::HARPOON_LEFT_RIGHT` | `⇋ ` | `U+21CB` | `<->` |
| | `Icons::HARPOON_RIGHT_LEFT` / `HARPOON_EXCHANGE` | `⇌ ` | `U+21CC` | `<->` |
| **Supplemental Arrows-B** | `Icons::HARPOON_UP_DOWN` | `⥮ ` | `U+296E` | `^v` |
| | `Icons::HARPOON_DOWN_UP` | `⥯ ` | `U+296F` | `v^` |
| | `Icons::ARROW_TO_BAR_TOP` | `⤒ ` | `U+2912` | `\|^` |
| | `Icons::ARROW_TO_BAR_BOTTOM` | `⤓ ` | `U+2913` | `\|v` |
| | `Icons::ARROW_DIAGONAL_NW_SE` | `⤡ ` | `U+2921` | `\` |
| | `Icons::ARROW_DIAGONAL_NE_SW` | `⤢ ` | `U+2922` | `/` |
| | `Icons::ARROW_CURVE_DOWN_LEFT` | `⤶ ` | `U+2936` | `<,` |
| | `Icons::ARROW_CURVE_DOWN_RIGHT` | `⤷ ` | `U+2937` | `,>` |
| **Flags & Milestones** | `Icons::FLAG_FILLED` | `⚑ ` | `U+2691` | `[F]` |
| | `Icons::FLAG_EMPTY` | `⚐ ` | `U+2690` | `[f]` |
| **Celestial & Stars** | `Icons::SUN` | `☼ ` | `U+263C` | `*` |
| | `Icons::STAR_FILLED` | `★ ` | `U+2605` | `[*]` |
| | `Icons::STAR_EMPTY` | `☆ ` | `U+2606` | `[ ]` |
| | `Icons::LIGHTNING` | `☇ ` | `U+2607` | `/` |
| **Topology & Links** | `Icons::NODE_LINK` | `☌ ` | `U+260C` | `o-` |
| | `Icons::PEER_LINK` | `☍ ` | `U+260D` | `o-o` |
| **Ornaments & Angles** | `Icons::BRACKET_HEAVY_LEFT` / `RIGHT` | `❰ ` / `❱ ` | `U+2770` / `U+2771` | `<` / `>` |
| | `Icons::BRACKET_MEDIUM_LEFT` / `RIGHT` | `❬ ` / `❭ ` | `U+276C` / `U+276D` | `<` / `>` |
| | `Icons::QUOTE_ANGLE_LEFT` / `RIGHT` | `❮ ` / `❯ ` | `U+276E` / `U+276F` | `<` / `>` |

> **Bảo vệ thị giác (Visual Protection)**:
> 1. Toàn bộ icon được chuẩn hóa đúng 2 cell hiển thị (1 ký tự glyph + 1 khoảng trắng đệm) để tránh phân mảnh heap và thẳng hàng tuyệt đối.
> 2. `Icons::span()` và `Icons::line()` tự động triệt tiêu cờ `Modifier::BOLD` riêng cho ký tự icon, ngăn chặn hiện tượng co rút 1 cell trên Windows Terminal / conhost.

### 7.1. Hệ Thống Tiền Tố Cố Định (Fixed-Width Tag System `[Tags]`) `[x]`

Nhằm giải quyết triệt để bài toán **Layout Shift (giật dòng răng cưa)** khi render streaming logs, status bar, tree view, hệ thống quy chuẩn thành 2 form cố định:

#### 1. Chuẩn Text-based 4 chữ: `[XXXX]` (Độ rộng chính xác: 6 cell)

| Danh mục | Tag `[XXXX]` | Hằng số Rust | Ý nghĩa / Ngữ cảnh |
| :--- | :--- | :--- | :--- |
| **Trạng thái & Sự kiện** | `[INFO]` | `Tags::INFO` | Thông tin điều hướng chung |
| | `[WARN]` | `Tags::WARN` | Cảnh báo tài nguyên / nghiệp vụ |
| | `[FAIL]` | `Tags::FAIL` | Thất bại / Lỗi nghiêm trọng |
| | `[PASS]` | `Tags::PASS` | Hợp lệ / Kiểm thử thành công |
| | `[DONE]` | `Tags::DONE` | Hoàn tất tác vụ |
| | `[WAIT]` | `Tags::WAIT` | Đang xếp hàng / Chờ tài nguyên |
| | `[IDLE]` | `Tags::IDLE` | Trạng thái nghỉ / Sẵn sàng |
| | `[BUSY]` | `Tags::BUSY` | Đang bận xử lý dữ liệu |
| **Cấu hình & Hệ thống** | `[CONF]` | `Tags::CONF` | Cài đặt / Tệp cấu hình |
| | `[CORE]` | `Tags::CORE` | Luồng nhân lõi / Engine |
| | `[DEVS]` | `Tags::DEVS` | Môi trường Dev / Thiết bị |
| | `[TEST]` | `Tags::TEST` | Kiểm thử tự động / Benchmark |
| | `[EXEC]` | `Tags::EXEC` | Lệnh thực thi / Binary run |
| **Tệp tin & Lưu trữ** | `[DIRS]` | `Tags::DIRS` | Thư mục phân cấp |
| | `[FILE]` | `Tags::FILE` | Tệp dữ liệu đơn |
| | `[DOCS]` | `Tags::DOCS` | Tài liệu / Ghi chú |
| | `[PACK]` | `Tags::PACK` | Gói cài đặt / Dependencies |
| | `[ARCH]` | `Tags::ARCH` | Kho lưu trữ / File nén |
| **Media & Âm thanh** | `[MEDA]` | `Tags::MEDA` | Đa phương tiện chung |
| | `[AUDI]` | `Tags::AUDI` | Luồng âm thanh / Track nhạc |
| | `[RECD]` | `Tags::RECD` | Đang thu âm / Micro bật |
| | `[SONG]` | `Tags::SONG` | Bài hát / Danh sách phát |
| | `[VOIC]` | `Tags::VOIC` | Giọng nói AI / Dubbing |
| **Mạng & Bảo mật** | `[NETW]` | `Tags::NETW` | Kết nối mạng / Socket |
| | `[APIS]` | `Tags::APIS` | Endpoint REST / RPC |
| | `[AUTH]` | `Tags::AUTH` | Xác thực / Quyền hạn / Token |
| | `[NODE]` | `Tags::NODE` | Máy chủ Node / Cluster |
| | `[PORT]` | `Tags::PORT` | Cổng dịch vụ kết nối |

#### 2. Chuẩn Symbol-based 1 ký tự: `[X]` (Độ rộng chính xác: 3 cell)

| Danh mục | Tag `[X]` | Hằng số Rust | Ý nghĩa / Ngữ cảnh |
| :--- | :--- | :--- | :--- |
| **Điều hướng & Cây** | `[>]` | `Tags::POINTER` | Con trỏ chọn / Active |
| | `[+]` | `Tags::EXPAND` | Thư mục đóng (bấm mở) |
| | `[-]` | `Tags::COLLAPSE` | Thư mục mở (bấm thu) |
| | `[/]` | `Tags::PATH` | Thư mục con phân cấp |
| | `[~]` | `Tags::HOME` | Thư mục gốc / Home |
| **Lựa chọn Form** | `[x]` | `Tags::CHECKED` | Checkbox đã chọn |
| | `[ ]` | `Tags::UNCHECKED` | Checkbox chưa chọn |
| | `[*]` | `Tags::RADIO_ACTIVE` | Radio đã chọn |
| | `[.]` | `Tags::RADIO_INACTIVE` | Radio chưa chọn |
| **Thông báo & Trợ giúp** | `[i]` | `Tags::INFO_CHAR` | Trợ giúp thông tin |
| | `[!]` | `Tags::ALERT_CHAR` | Cảnh báo khẩn cấp |
| | `[?]` | `Tags::HELP_CHAR` | Câu hỏi xác nhận |
| | `[#]` | `Tags::ERROR_CHAR` | Lỗi xung đột / Mã số |

---

## 8. Animation & Motion `[x]`

### Spinners `[x]` (`TaskWidget`)

- **Pulse Spinner (Thinking/AI)**: Chu kỳ 150ms qua các trạng thái `· ` → `• ` → `● ` → `• ` → `· `.
- **Braille 6-Frame Spinner (Build/IO)**: Chu kỳ 80ms qua 6 khung viền kinh điển `⠋` → `⠙` → `⠸` → `⠴` → `⠦` → `⠇` (`SpinnerType::Braille6`).
- **Braille 10-Frame Spinner (Docker)**: Chu kỳ 80ms qua các khung hình `⠋`, `⠙`, `⠹`, `⠸`, `⠼`, `⠴`, `⠦`, `⠧`, `⠇`, `⠏` (`SpinnerType::Dots`).
- **Moon Spinner (Clockwise Rotating Circle)**: Chu kỳ 120ms qua 4 pha bán cầu xoay tròn `◐` → `◒` → `◑` → `◓` (`SpinnerType::Moon`). Khớp chuẩn 1 cell monospace trên Windows Terminal / Powershell.
- **Arc Spinner (Quadrant Circular Arc)**: Chu kỳ 100ms qua 4 góc cung tròn `◜` → `◝` → `◞` → `◟` (`SpinnerType::Arc`). Khớp chuẩn 1 cell monospace trên Windows Terminal / Powershell.

### Progress Bars `[x]` (`TaskWidget`)

```
  Line:           ━━━━━━━━━━────────── 50% [250/500 units] ─ 12s
  Parallelogram:  ▰▰▰▰▰▰▰▰▰▰▱▱▱▱▱▱▱▱▱▱ 50% [250/500 units] ─ 12s
```

- Thanh đo lường tiến trình chính xác, hỗ trợ chuyển đổi mượt mà từ Spinner bất định sang Progress có đo lường.
- Hỗ trợ 2 kiểu ký tự chuẩn hóa qua enum `ProgressStyle`:
  - `ProgressStyle::Line`: `━` / `─` (Vạch mảnh thanh thoát, tối giản Vercel).
  - `ProgressStyle::Parallelogram`: `▰` / `▱` (Hình bình hành xiên hiện đại, phong cách Cyberpunk/CLI).

### Shimmer Bar `[x]` (`ShimmerWidget`)

```
  Linear Sync Engine ─ Background Delta Syncing...
```

- Hiệu ứng quét sáng sóng gradient tuyến tính 60 FPS, mô phỏng tải nền kiểu Linear.

---

## 9. Nguyên tắc cốt lõi (Core Principles)

- **Đơn giản, chuẩn xác**: Không màu mè quá mức, dùng màu sắc phục vụ đúng ngữ cảnh nghiệp vụ.
- **Không dùng Emoji**: Emoji có bề rộng không cố định ở các terminal khác nhau gây vỡ layout. Chỉ dùng Unicode Text Symbols.
- **Tập trung vào hiệu suất**: Zero-heap allocation trong vòng lặp render, tối ưu hóa triệt để tài nguyên terminal.
