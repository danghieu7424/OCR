# Graph Report - OCR  (2026-11-14)

## Corpus Check
- 60 files · ~87,805 words
- Verdict: corpus is large enough that graph structure adds value.
- Unclassified: 8 file(s) not represented in the graph (top: (none) 5, .onnx 2, .zip 1)

## Summary
- 1258 nodes · 2337 edges · 61 communities (37 shown, 24 thin omitted)
- Extraction: 100% EXTRACTED · 0% INFERRED · 0% AMBIGUOUS · INFERRED: 7 edges (avg confidence: 0.81)
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `5ccff8d3`
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
- ocr_shared/src/lib.rs
- Minimal — TUI Design System
- editable_list.rs
- ShimmerWidget
- ListWidget
- ButtonWidget
- compare_subs.py
- det_filter.rs
- icons.rs
- ollama.rs
- cli_gui_form
- TaskShared
- 🚀 Hướng dẫn sử dụng
- cli_gui_form/src/lib.rs
- extract_captions.rs
- [0.2.0] — 2026-11-14
- extract_subtitles.rs
- OCR_Subtitles/src/lib.rs
- TaskShared
- CardWidget
- proofread.rs
- TaskWidget
- tags.rs
- TaskShared
- plan.md
- OCR_Captions/src/lib.rs
- TaskShared
- task.rs
- StatusBarWidget
- TaskShared
- EventResult
- OCR
- evaluate.md
- WorkerMessage
- WorkerMessage
- src/main.rs
- SpinnerType
- FormWidget
- CheckboxWidget
- EntityGlossary
- Proofreader
- RadioWidget
- FormManager
- proofread_subtitles.rs
- OCR_Merge/src/lib.rs
- temporal_voting.rs
- DeploymentItem

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
- `main()` --calls--> `merge_vtt_files()`  [INFERRED]
  examples/merge_subtitles_captions.rs → crates/OCR_Merge/src/lib.rs
- `OCR` --depends_on--> `cli_gui_form`  [EXTRACTED]
  Cargo.toml → cli_gui_form/Cargo.toml
- `OCR` --depends_on--> `OCR_Merge`  [EXTRACTED]
  Cargo.toml → crates/OCR_Merge/Cargo.toml
- `ocr_shared` --depends_on--> `cli_gui_form`  [EXTRACTED]
  crates/ocr_shared/Cargo.toml → cli_gui_form/Cargo.toml
- `OCR` --depends_on--> `OCR_Captions`  [EXTRACTED]
  Cargo.toml → crates/OCR_Captions/Cargo.toml

## Import Cycles
- None detected.

## Communities (61 total, 24 thin omitted)

### Community 0 - "Icons"
Cohesion: 0.01
Nodes (148): Icons, .APPROX, .ARC_BOTTOM_LEFT, .ARC_BOTTOM_RIGHT, .ARC_TOP_LEFT, .ARC_TOP_RIGHT, .ARROW_CURVE_DOWN_LEFT, .ARROW_CURVE_DOWN_RIGHT (+140 more)

### Community 2 - "Tags"
Cohesion: 0.03
Nodes (71): Tags, .ALERT_CHAR, .APIS, .ARCH, .AUDI, .AUTH, .BUSY, .CERT (+63 more)

### Community 3 - "Theme"
Cohesion: 0.05
Nodes (68): Theme, .ACCENT, .BG, .BLACK, .BORDER_FOCUS, .BTN_ACCENT, .BTN_AMBER, .BTN_EMERALD (+60 more)

### Community 4 - "InputWidget"
Cohesion: 0.11
Nodes (6): InputMode, Password, Text, InputWidget, test_input_placeholder_builder(), test_input_typing_and_backspace()

### Community 5 - "TabsWidget"
Cohesion: 0.08
Nodes (6): TabsWidget, test_tabs_auto_scroll_sticky(), test_tabs_container_overflow_protection(), test_tabs_mouse_and_step_scrolling(), test_tabs_navigation(), test_tabs_scrolling()

### Community 6 - "SelectWidget"
Cohesion: 0.08
Nodes (8): SelectOption, SelectWidget, test_select_widget_basic_creation_and_value(), test_select_widget_dozens_of_options_scrolling_and_navigation(), test_select_widget_esc_cancels(), test_select_widget_open_navigate_and_select(), test_truncate_with_ellipsis_and_overflow_protection(), truncate_with_ellipsis()

### Community 7 - "ocr_shared/src/lib.rs"
Cohesion: 0.09
Nodes (24): attach_cli_gui_task(), clear_handlers(), CpuOptimizer, emit_log(), emit_step(), emit_task_progress(), EventHandlerRegistry, get_registry() (+16 more)

### Community 8 - "Minimal — TUI Design System"
Cohesion: 0.06
Nodes (34): 1. Chuẩn Text-based 4 chữ: `[XXXX]` (Độ rộng chính xác: 6 cell), 1. Theme Overview, 2. Chuẩn Symbol-based 1 ký tự: `[X]` (Độ rộng chính xác: 3 cell), 2. Color Palette `[x]`, 3. Typography & ASCII Art `[x]`, 4. Borders & Box Drawing `[x]`, 5. Components `[x]`, 6. Layout & Spacing `[x]` (+26 more)

### Community 9 - "editable_list.rs"
Cohesion: 0.11
Nodes (7): EditableListWidget, test_editable_list_add_item_workflow(), test_editable_list_creation_and_value(), test_editable_list_delete_item_workflow(), test_editable_list_edit_item_workflow(), test_editable_list_esc_key_hierarchy(), truncate_with_ellipsis()

### Community 10 - "ShimmerWidget"
Cohesion: 0.07
Nodes (4): main(), ShimmerShared, ShimmerWidget, test_shimmer_tick_cycle()

### Community 11 - "ListWidget"
Cohesion: 0.10
Nodes (4): ListItem, ListWidget, test_list_widget_form_value(), test_list_widget_sub_level_navigation()

### Community 12 - "ButtonWidget"
Cohesion: 0.13
Nodes (4): ButtonWidget, test_button_default_no_bold_clean_rendering(), test_button_icon_never_bold_while_text_is_bold(), test_button_with_explicit_icon_separation()

### Community 13 - "compare_subs.py"
Cohesion: 0.23
Nodes (7): align(), levenshtein(), main(), make_normalizer(), norm(), parse_vtt(), to_sec()

### Community 14 - "det_filter.rs"
Cohesion: 0.39
Nodes (3): create_det_engine(), create_det_engine_with_limit(), extract_patch_with_det()

### Community 17 - "cli_gui_form"
Cohesion: 0.11
Nodes (17): 1. `ListWidget` (Menu Phân Cấp & Điều Hướng Cây Con), 2. `RadioWidget` (Nhóm Radio Chọn Ngang Tinh Gọn), 3. `SelectWidget` (Dropdown Popup Overlay ─ HTML `<select>` / `<option>`), 4. `TabsWidget` & `StatusBarWidget` (Hệ Thống Thẻ & Cuộn Chuột), 4. `TaskWidget` (Vòng Đời Tác Vụ Đa Pha & Animation), 5. `CardWidget` & `ShimmerWidget`, Biểu Tượng Chuẩn (`Icons`), Bảng Màu Hệ Thống (`Theme`) (+9 more)

### Community 19 - "🚀 Hướng dẫn sử dụng"
Cohesion: 0.13
Nodes (14): 1. Dùng như Thư viện Rust (`--lib`), 🔍 1. Tải mô hình phát hiện vị trí chữ (`det.onnx`), 2. Tích hợp thanh tiến trình với `cli_gui_form` (TUI Ready), 🔤 2. Từ điển ký tự (`keys.txt`), 3. Chạy file ví dụ mẫu (`examples/`), 📥 3. Tải mô hình nhận diện chữ (`rec.onnx`), 4. Chạy chế độ hàng loạt CLI (`--bin`), Cách A: Cắm trực tiếp vào `cli_gui_form::TaskWidget` (+6 more)

### Community 20 - "cli_gui_form/src/lib.rs"
Cohesion: 0.20
Nodes (10): EditMode, Adding, Browsing, Editing, ProgressStyle, Line, Parallelogram, TaskState (+2 more)

### Community 21 - "extract_captions.rs"
Cohesion: 0.40
Nodes (8): caption_path(), download_file(), ensure_models(), extract_and_report(), is_done(), main(), run_batch(), run_one()

### Community 22 - "[0.2.0] — 2026-11-14"
Cohesion: 0.07
Nodes (28): [0.1.0] — 2026-11-05, [0.2.0] — 2026-11-14, [0.2.1] — 2026-11-14, [0.3.0] — 2026-11-14, [0.4.1] — 2026-11-14, [0.4.2] — 2026-11-14, [0.5.0] — 2026-11-14, Bảo mật (+20 more)

### Community 23 - "extract_subtitles.rs"
Cohesion: 0.32
Nodes (3): download_model_file(), ensure_models_exist(), main()

### Community 24 - "OCR_Subtitles/src/lib.rs"
Cohesion: 0.15
Nodes (12): common_prefix_len(), common_suffix_len(), extract_hardcoded_subtitles(), is_same_subtitle(), levenshtein_distance(), OcrExtractorConfig, OcrExtractorOutput, PpOcrEngine (+4 more)

### Community 26 - "CardWidget"
Cohesion: 0.24
Nodes (3): CardItem, CardWidget, test_card_widget_creation()

### Community 31 - "plan.md"
Cohesion: 0.15
Nodes (12): 1. Dùng LLM Hậu xử lý (Post-processing Context Correction), 1. Mục 1: Tích hợp Qwen2.5-VL-3B-Instruct (Hậu xử lý trên CPU), 1. Về LLM Hậu xử lý (Chạy thuần CPU, cực nhẹ, đọc tốt tiếng Trung và nhận được ảnh - VLM), 2. Mục 2: Tự động tạo Entity Glossary từ Caption, 2. Tận dụng từ điển danh từ riêng & Nhân vật (Glossary / Entity Dictionary), 2. Tự động hóa tạo Danh từ riêng / Nhân vật (Glossary hoàn toàn tự động), 3. Cơ chế "Bỏ phiếu đa khung hình" (Multi-frame Temporal Voting), 3. Mục 3: Nâng cấp Bỏ phiếu đa khung hình (Temporal Voting cho Subtitle) (+4 more)

### Community 32 - "OCR_Captions/src/lib.rs"
Cohesion: 0.13
Nodes (27): assemble_caption(), box_pixel_stats(), BoxPixels, build_watermark_set(), CAPTION_BRIGHT_BG_LUM, caption_key(), CAPTION_MAX_INK_SAT, CAPTION_MIN_SHADOW_FRAC (+19 more)

### Community 34 - "task.rs"
Cohesion: 0.15
Nodes (5): SPINNER_FRAMES_ARC, SPINNER_FRAMES_BRAILLE_6, SPINNER_FRAMES_DOTS, SPINNER_FRAMES_MOON, SPINNER_FRAMES_PULSE

### Community 37 - "EventResult"
Cohesion: 0.21
Nodes (6): EventResult, Consumed, Ignored, Submitted, render_input_spans(), test_render_input_spans_block_cursor()

### Community 38 - "OCR"
Cohesion: 0.52
Nodes (7): cli_gui_form, OCR, OCR_Captions, OCR_LLM, OCR_Merge, ocr_shared, OCR_Subtitles

### Community 40 - "evaluate.md"
Cohesion: 0.05
Nodes (42): 10 lỗi thật còn lại (sau t2s), 1. Dùng LLM Hậu xử lý (Post-processing Context Correction), 1. Module `src/glossary.rs` — `EntityGlossary` (Tự động tạo Danh từ riêng từ Caption), 1. Subtitle (20 tập) — `output/第XXX集.zh.vtt` vs reference `input/第XXX集.zh.vtt`, 1. Tổng quan kết quả đối chiếu 4 tập đầu (`final/` vs `output/`), 2. Caption — đối chiếu reference (tập 1–4), 2. Module `src/temporal_voting.rs` (Bỏ phiếu đa khung hình cấp ký tự), 2. Soi frame hình ảnh thực tế cắt từ video (+34 more)

### Community 42 - "WorkerMessage"
Cohesion: 0.67
Nodes (3): WorkerMessage, ModelLoaded, ProgressUpdate

### Community 43 - "WorkerMessage"
Cohesion: 0.67
Nodes (3): WorkerMessage, ModelLoaded, ProgressUpdate

### Community 45 - "src/main.rs"
Cohesion: 0.23
Nodes (5): init_logger(), LOG_FILE, log_eprintln!, log_println!, main()

### Community 47 - "SpinnerType"
Cohesion: 0.21
Nodes (8): SpinnerType, Arc, Braille6, Dots, Moon, Pulse, test_task_loading_tick_with_moon(), test_task_loading_tick_with_pulse()

### Community 48 - "FormWidget"
Cohesion: 0.11
Nodes (7): FormValue, Bool, List, None, Select, Text, FormWidget

### Community 50 - "EntityGlossary"
Cohesion: 0.21
Nodes (4): EntityGlossary, levenshtein_chars(), test_extract_from_vtt(), test_normalize_sentence()

### Community 51 - "Proofreader"
Cohesion: 0.21
Nodes (4): ProofreadConfig, Proofreader, test_generic_sanitizer_and_custom_rules(), test_proofread_with_glossary()

### Community 55 - "OCR_Merge/src/lib.rs"
Cohesion: 0.42
Nodes (10): format_vtt_timestamp(), merge_cues(), merge_vtt_files(), MergeSummary, parse_vtt(), parse_vtt_timestamp(), serialize_vtt(), test_merge_cues_ordering() (+2 more)

### Community 56 - "temporal_voting.rs"
Cohesion: 0.36
Nodes (7): char_similarity(), levenshtein(), test_vote_character_consensus_bua_chu(), test_vote_character_consensus_noise_removal(), test_vote_majority_string(), vote_character_consensus(), vote_majority_string()

## Knowledge Gaps
- **25 isolated node(s):** `Consumed`, `Ignored`, `Submitted`, `Bool`, `List` (+20 more)
  These have ≤1 connection - possible missing edges or undocumented components. (Counts symbols only; 579 node(s) total have ≤1 connection when file, concept and rationale nodes are included.)
- **24 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `Icons` connect `Icons` to `icons.rs`?**
  _High betweenness centrality (0.180) - this node is a cross-community bridge._
- **What connects `Consumed`, `Ignored`, `Submitted` to the rest of the system?**
  _25 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `Icons` be split into smaller, more focused modules?**
  _Cohesion score 0.013422818791946308 - nodes in this community are weakly interconnected._
- **Why does `Tags` connect `Tags` to `demo_master.rs`, `cli_gui_form/src/lib.rs`, `tags.rs`?**
  _High betweenness centrality (0.112) - this node is a cross-community bridge._
- **Should `Tags` be split into smaller, more focused modules?**
  _Cohesion score 0.027777777777777776 - nodes in this community are weakly interconnected._
- **Why does `FormWidget` connect `FormWidget` to `demo_master.rs`, `InputWidget`, `TabsWidget`, `SelectWidget`, `editable_list.rs`, `ShimmerWidget`, `ListWidget`, `ButtonWidget`, `TaskShared`, `cli_gui_form/src/lib.rs`, `TaskShared`, `TaskWidget`, `TaskShared`, `TaskShared`, `task.rs`, `TaskShared`, `EventResult`, `CheckboxWidget`, `RadioWidget`, `FormManager`?**
  _High betweenness centrality (0.101) - this node is a cross-community bridge._
- **Should `Theme` be split into smaller, more focused modules?**
  _Cohesion score 0.0539906103286385 - nodes in this community are weakly interconnected._