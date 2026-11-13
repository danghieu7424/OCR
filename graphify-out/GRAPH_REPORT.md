# Graph Report - OCR  (2026-11-14)

## Corpus Check
- 48 files · ~74,441 words
- Verdict: corpus is large enough that graph structure adds value.
- Unclassified: 8 file(s) not represented in the graph (top: (none) 5, .onnx 2, .zip 1)

## Summary
- 1092 nodes · 2086 edges · 38 communities (27 shown, 11 thin omitted)
- Extraction: 100% EXTRACTED · 0% INFERRED · 0% AMBIGUOUS · INFERRED: 6 edges (avg confidence: 0.8)
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `d3f33d9d`
- Run `git rev-parse HEAD` and compare to check if the graph is stale.
- Run `graphify update .` after code changes (no API cost).

## Community Hubs (Navigation)
- Icons
- demo_master.rs
- Tags
- Theme
- InputWidget
- TabsWidget
- SelectWidget
- shared.rs
- Minimal — TUI Design System
- editable_list.rs
- ShimmerWidget
- cli_gui_form/src/lib.rs
- ButtonWidget
- compare_subs.py
- caption.rs
- icons.rs
- OCR/mod.rs
- cli_gui_form
- TaskShared
- 🚀 Hướng dẫn sử dụng
- TaskShared
- [0.2.0] — 2026-11-14
- TaskShared
- TaskWidget
- StatusBarWidget
- extract_captions.rs
- src/main.rs
- tags.rs
- TaskShared
- [MODULE]: Video Hardcoded Subtitle OCR (Trích xuất phụ đề cứng từ Video)
- det_filter.rs
- extract_subtitles.rs
- plan.md
- cli_gui_form
- evaluate.md
- WorkerMessage
- WorkerMessage

## God Nodes (most connected - your core abstractions)
1. `Icons` - 159 edges
2. `Theme` - 82 edges
3. `Tags` - 77 edges
4. `FormWidget` - 48 edges
5. `EventResult` - 42 edges
6. `TaskWidget` - 34 edges
7. `TabsWidget` - 33 edges
8. `FormValue` - 30 edges
9. `SelectWidget` - 29 edges
10. `ButtonWidget` - 28 edges

## Surprising Connections (you probably didn't know these)
- `extract_and_report()` --calls--> `extract_captions()`  [INFERRED]
  examples/extract_captions.rs → src/caption.rs
- `main()` --calls--> `extract_hardcoded_subtitles()`  [INFERRED]
  examples/extract_subtitles.rs → src/OCR/mod.rs
- `ShimmerShared` --implements--> `FormWidget`  [INFERRED]
  cli_gui_form/examples/status_shimmer.rs → cli_gui_form/src/traits.rs
- `extract_and_report()` --references--> `CaptionExtractorConfig`  [EXTRACTED]
  examples/extract_captions.rs → src/caption.rs
- `run_batch()` --references--> `CaptionExtractorConfig`  [EXTRACTED]
  examples/extract_captions.rs → src/caption.rs

## Import Cycles
- None detected.

## Communities (38 total, 11 thin omitted)

### Community 0 - "Icons"
Cohesion: 0.01
Nodes (148): Icons, .APPROX, .ARC_BOTTOM_LEFT, .ARC_BOTTOM_RIGHT, .ARC_TOP_LEFT, .ARC_TOP_RIGHT, .ARROW_CURVE_DOWN_LEFT, .ARROW_CURVE_DOWN_RIGHT (+140 more)

### Community 1 - "demo_master.rs"
Cohesion: 0.05
Nodes (22): TableRowItem, DeploymentItem, FormManager, test_form_manager_focus_navigation(), EventResult, Consumed, Ignored, Submitted (+14 more)

### Community 2 - "Tags"
Cohesion: 0.03
Nodes (71): Tags, .ALERT_CHAR, .APIS, .ARCH, .AUDI, .AUTH, .BUSY, .CERT (+63 more)

### Community 3 - "Theme"
Cohesion: 0.05
Nodes (68): Theme, .ACCENT, .BG, .BLACK, .BORDER_FOCUS, .BTN_ACCENT, .BTN_AMBER, .BTN_EMERALD (+60 more)

### Community 4 - "InputWidget"
Cohesion: 0.11
Nodes (4): Text, InputWidget, test_input_placeholder_builder(), test_input_typing_and_backspace()

### Community 5 - "TabsWidget"
Cohesion: 0.09
Nodes (6): TabsWidget, test_tabs_auto_scroll_sticky(), test_tabs_container_overflow_protection(), test_tabs_mouse_and_step_scrolling(), test_tabs_navigation(), test_tabs_scrolling()

### Community 6 - "SelectWidget"
Cohesion: 0.08
Nodes (8): SelectOption, SelectWidget, test_select_widget_basic_creation_and_value(), test_select_widget_dozens_of_options_scrolling_and_navigation(), test_select_widget_esc_cancels(), test_select_widget_open_navigate_and_select(), test_truncate_with_ellipsis_and_overflow_protection(), truncate_with_ellipsis()

### Community 7 - "shared.rs"
Cohesion: 0.10
Nodes (22): attach_cli_gui_task(), clear_handlers(), CpuOptimizer, emit_log(), emit_step(), emit_task_progress(), EventHandlerRegistry, get_registry() (+14 more)

### Community 8 - "Minimal — TUI Design System"
Cohesion: 0.06
Nodes (34): 1. Chuẩn Text-based 4 chữ: `[XXXX]` (Độ rộng chính xác: 6 cell), 1. Theme Overview, 2. Chuẩn Symbol-based 1 ký tự: `[X]` (Độ rộng chính xác: 3 cell), 2. Color Palette `[x]`, 3. Typography & ASCII Art `[x]`, 4. Borders & Box Drawing `[x]`, 5. Components `[x]`, 6. Layout & Spacing `[x]` (+26 more)

### Community 9 - "editable_list.rs"
Cohesion: 0.11
Nodes (9): EditableListWidget, render_input_spans(), test_editable_list_add_item_workflow(), test_editable_list_creation_and_value(), test_editable_list_delete_item_workflow(), test_editable_list_edit_item_workflow(), test_editable_list_esc_key_hierarchy(), test_render_input_spans_block_cursor() (+1 more)

### Community 10 - "ShimmerWidget"
Cohesion: 0.07
Nodes (4): main(), ShimmerShared, ShimmerWidget, test_shimmer_tick_cycle()

### Community 11 - "cli_gui_form/src/lib.rs"
Cohesion: 0.06
Nodes (14): CardItem, CardWidget, test_card_widget_creation(), EditMode, Adding, Browsing, Editing, ListItem (+6 more)

### Community 12 - "ButtonWidget"
Cohesion: 0.13
Nodes (4): ButtonWidget, test_button_default_no_bold_clean_rendering(), test_button_icon_never_bold_while_text_is_bold(), test_button_with_explicit_icon_separation()

### Community 13 - "compare_subs.py"
Cohesion: 0.23
Nodes (7): align(), levenshtein(), main(), make_normalizer(), norm(), parse_vtt(), to_sec()

### Community 14 - "caption.rs"
Cohesion: 0.15
Nodes (25): assemble_caption(), box_pixel_stats(), BoxPixels, build_watermark_set(), CAPTION_BRIGHT_BG_LUM, caption_key(), CAPTION_MAX_INK_SAT, CAPTION_MIN_SHADOW_FRAC (+17 more)

### Community 16 - "OCR/mod.rs"
Cohesion: 0.13
Nodes (12): common_prefix_len(), common_suffix_len(), extract_hardcoded_subtitles(), is_same_subtitle(), levenshtein_distance(), OcrExtractorConfig, OcrExtractorOutput, PpOcrEngine (+4 more)

### Community 17 - "cli_gui_form"
Cohesion: 0.11
Nodes (17): 1. `ListWidget` (Menu Phân Cấp & Điều Hướng Cây Con), 2. `RadioWidget` (Nhóm Radio Chọn Ngang Tinh Gọn), 3. `SelectWidget` (Dropdown Popup Overlay ─ HTML `<select>` / `<option>`), 4. `TabsWidget` & `StatusBarWidget` (Hệ Thống Thẻ & Cuộn Chuột), 4. `TaskWidget` (Vòng Đời Tác Vụ Đa Pha & Animation), 5. `CardWidget` & `ShimmerWidget`, Biểu Tượng Chuẩn (`Icons`), Bảng Màu Hệ Thống (`Theme`) (+9 more)

### Community 19 - "🚀 Hướng dẫn sử dụng"
Cohesion: 0.13
Nodes (14): 1. Dùng như Thư viện Rust (`--lib`), 🔍 1. Tải mô hình phát hiện vị trí chữ (`det.onnx`), 2. Tích hợp thanh tiến trình với `cli_gui_form` (TUI Ready), 🔤 2. Từ điển ký tự (`keys.txt`), 3. Chạy file ví dụ mẫu (`examples/`), 📥 3. Tải mô hình nhận diện chữ (`rec.onnx`), 4. Chạy chế độ hàng loạt CLI (`--bin`), Cách A: Cắm trực tiếp vào `cli_gui_form::TaskWidget` (+6 more)

### Community 22 - "[0.2.0] — 2026-11-14"
Cohesion: 0.12
Nodes (16): [0.1.0] — 2026-11-05, [0.2.0] — 2026-11-14, [0.2.1] — 2026-11-14, Bảo mật, Changelog, Gỡ bỏ, Kiểm chứng, Sửa (+8 more)

### Community 25 - "TaskWidget"
Cohesion: 0.05
Nodes (20): main(), TaskShared, ProgressStyle, Line, Parallelogram, SPINNER_FRAMES_ARC, SPINNER_FRAMES_BRAILLE_6, SPINNER_FRAMES_DOTS (+12 more)

### Community 27 - "extract_captions.rs"
Cohesion: 0.29
Nodes (10): caption_path(), download_file(), ensure_models(), extract_and_report(), is_done(), main(), run_batch(), run_one() (+2 more)

### Community 28 - "src/main.rs"
Cohesion: 0.21
Nodes (5): init_logger(), LOG_FILE, log_eprintln!, log_println!, main()

### Community 32 - "[MODULE]: Video Hardcoded Subtitle OCR (Trích xuất phụ đề cứng từ Video)"
Cohesion: 0.29
Nodes (6): I. KIẾN TRÚC LUỒNG XỬ LÝ (4-STEP PIPELINE), II. ĐẶC TẢ THÔNG SỐ KỸ THUẬT & BẢO VỆ RANH GIỚI (GUARDRAILS), III. THỰC NGHIỆM TRÊN VIDEO MẪU & BỘ PHỤ ĐỀ CHUẨN, IV. BẢNG ĐỐI SOÁNH ĐỘ CHUẨN XÁC VỀ THỜI GIAN & NỘI DUNG (BENCHMARK), [MODULE]: Video Hardcoded Subtitle OCR (Trích xuất phụ đề cứng từ Video), V. ĐÁNH GIÁ CHỈ SỐ KỸ THUẬT (METRICS ANALYSIS)

### Community 35 - "extract_subtitles.rs"
Cohesion: 0.22
Nodes (3): download_model_file(), ensure_models_exist(), main()

### Community 40 - "evaluate.md"
Cohesion: 0.25
Nodes (7): 1. Subtitle (20 tập) — `output/第XXX集.zh.vtt` vs reference `input/第XXX集.zh.vtt`, 2. Caption — đối chiếu reference (tập 1–4), 3. Caption tập 5–20 (không có reference) — kiểm tra bằng ảnh tại vị trí caption, 4. Ghi chú vận hành, Kết quả chạy lại (001–009, 012), Luật mới (`src/caption.rs`), Sách/commit

### Community 42 - "WorkerMessage"
Cohesion: 0.67
Nodes (3): WorkerMessage, ModelLoaded, ProgressUpdate

### Community 43 - "WorkerMessage"
Cohesion: 0.67
Nodes (3): WorkerMessage, ModelLoaded, ProgressUpdate

## Knowledge Gaps
- **27 isolated node(s):** `Consumed`, `Ignored`, `Submitted`, `Password`, `cli_gui_form` (+22 more)
  These have ≤1 connection - possible missing edges or undocumented components. (Counts symbols only; 505 node(s) total have ≤1 connection when file, concept and rationale nodes are included.)
- **11 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `Icons` connect `Icons` to `icons.rs`?**
  _High betweenness centrality (0.265) - this node is a cross-community bridge._
- **What connects `Consumed`, `Ignored`, `Submitted` to the rest of the system?**
  _27 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `Icons` be split into smaller, more focused modules?**
  _Cohesion score 0.013422818791946308 - nodes in this community are weakly interconnected._
- **Why does `FormWidget` connect `demo_master.rs` to `InputWidget`, `TabsWidget`, `SelectWidget`, `editable_list.rs`, `ShimmerWidget`, `cli_gui_form/src/lib.rs`, `ButtonWidget`, `TaskShared`, `TaskShared`, `TaskShared`, `TaskWidget`, `TaskShared`?**
  _High betweenness centrality (0.128) - this node is a cross-community bridge._
- **Should `demo_master.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.05149023638232271 - nodes in this community are weakly interconnected._
- **Why does `Tags` connect `Tags` to `demo_master.rs`, `cli_gui_form/src/lib.rs`, `tags.rs`?**
  _High betweenness centrality (0.108) - this node is a cross-community bridge._
- **Should `Tags` be split into smaller, more focused modules?**
  _Cohesion score 0.027777777777777776 - nodes in this community are weakly interconnected._