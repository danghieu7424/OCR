# Changelog

Mọi thay đổi đáng chú ý của dự án được ghi lại trong tệp này.

Định dạng theo [Keep a Changelog 1.1.0](https://keepachangelog.com/vi/1.1.0/) và
[Semantic Versioning](https://semver.org/lang/vi/).

## [Unreleased]

## [0.4.2] — 2026-11-14

### Gỡ bỏ

- Toàn bộ các chuỗi và quy tắc hard-code theo ngữ cảnh video cụ thể (`娘亲参`, `参以前`, `魔域之人で`, `一还只是`) trong `src/proofread.rs`.

### Thêm

- Phương thức `Proofreader::sanitize_ocr_noise`: bộ lọc làm sạch nhiễu detector tổng quát (loại bỏ ký tự rác ngoại lai `^`, `_`, `|`, `~`, `v`, `V` ở viền câu và kana rác dính đuôi từ background) độc lập với video.
- Trường `custom_replacements` trong `ProofreadConfig`: cho phép caller/người dùng tự do nạp bảng từ điển ánh xạ lỗi chính tả tùy ý vào pipeline mà không cần sửa code.

### Thay đổi

- Tái cấu trúc pipeline hiệu đính: giao toàn bộ phần phân tích ngữ cảnh, đại từ xưng hô và câu thoại tự nhiên cho mô hình ngôn ngữ **Qwen2.5-VL** (thông qua `OllamaManager`), kết hợp với bảng danh từ riêng động (`EntityGlossary`) học từ caption `《...》`.
- Chuẩn hóa các cặp chữ Hán tương đồng hình học kinh điển trong OCR (`自已` ↔ `自己`, `眼晴` ↔ `眼睛`) thành bảng hằng số dùng chung thay vì các khối `if-else` lồng ghép.

## [0.4.1] — 2026-11-14

### Sửa

- Ép buộc chế độ Pure CPU (`OLLAMA_LLM_LIBRARY=cpu`, `CUDA_VISIBLE_DEVICES=""`, `VK_ICD_FILENAMES=""`) khi khởi động `ollama serve` tự động trong `OllamaManager`, ngăn chặn tình trạng treo vô hạn do Ollama tự kích hoạt Vulkan trên card đồ họa đời cũ (như GeForce GT 710 VRAM 984MB).
- Thiết lập thời gian chờ timeout 10 giây cho từng câu gọi mô hình qua `ollama-rs` để bảo đảm pipeline không bao giờ bị nghẽn (tự động fallback sang Rule Engine nếu quá hạn).

### Thay đổi

- Tối ưu hóa hiệu năng: tái sử dụng Tokio Runtime đa luồng tập trung trong `OllamaManager` thay vì khởi tạo lại runtime trên từng câu.
- Bổ sung hiển thị tiến độ thời gian thực (real-time progress) theo từng dòng câu trong phương thức `Proofreader::proofread_vtt_file`.

### Thêm

- Module `src/glossary.rs` (`EntityGlossary`): tự động trích xuất danh sách danh từ riêng (tên nhân vật, công pháp, địa danh) từ các file `.captions.zh.vtt` và chuẩn hóa câu phụ đề, loại bỏ nguy cơ đọc nhầm tên riêng.
- Module `src/temporal_voting.rs`: thuật toán bỏ phiếu đa khung hình cấp chuỗi (`vote_majority_string`) và cấp ký tự (`vote_character_consensus`) giúp triệt tiêu nhiễu bùa chú, tia sáng lướt qua tạm thời giữa các frame.
- Module `src/ollama.rs` (`OllamaManager`): tích hợp thư viện chính thức `ollama-rs` của Rust, tự động định vị binary `ollama`, tự động khởi động daemon ngầm `ollama serve` nếu chưa chạy, và tự động kiểm tra/tải mô hình (`qwen2.5vl:3b`) qua API.
- Module `src/proofread.rs` (`Proofreader`): tầng hiệu đính ngữ cảnh hậu xử lý tích hợp trực tiếp `OllamaManager::with_ollama`, kết hợp luật ngữ cảnh nội bộ (sửa các cặp nhầm lẫn `参`↔`爹`, `涵`↔`嗯`, `自已`↔`自己`, xóa ký tự rác ngoại lai).
- Example `examples/proofread_subtitles.rs`: công cụ CLI hiệu đính phụ đề hàng loạt cho 20 tập từ từ điển caption tự động, tự kích hoạt và kết nối mô hình Qwen qua Ollama.

### Thay đổi

- `src/lib.rs`: re-export `EntityGlossary`, `Proofreader`, `ProofreadConfig`, `ProofreadSummary`, `OllamaManager`, `vote_character_consensus`, `vote_majority_string`.
- `Cargo.toml`: thêm dependencies `ollama-rs` và `tokio`, nâng phiên bản thư viện lên `0.4.0`.

## [0.3.0] — 2026-11-14

## [0.2.1] — 2026-11-14

### Sửa

- Loại false positive khi chữ nằm trên vật thể sáng trong cảnh: thêm kiểm tra thống kê pixel box (`BoxPixels` với `dark_frac`, `lum_med`). Box chỉ bị coi là chữ trong cảnh khi vừa thiếu quầng tối (`dark_frac < 0.20`) vừa có nền sáng (`lum_med > 100`) — đúng ca 《社》 trên băng rôn đỏ (tập 9: `dark_frac = 0.10`, `lum_med = 145–164`), trong khi caption thật có `dark_frac = 0.26–0.98`.

### Thay đổi

- Thêm log chẩn đoán bật bằng biến môi trường `OCR_CAP_DEBUG=1`: in `lum_med`, `dark_frac`, `sat_med`, `sat_p90`, `sat_hi` và kết quả từng bộ lọc cho mọi box ứng viên.

### Kiểm chứng

- Chạy lại tập 1–9 và 12: giữ nguyên 24 caption thật (001=7, 002=6, 003=3, 004=2, 005–008=1 mỗi tập, 009=1, 012=1), chữ không đổi; chỉ mất caption sai 《社》 ở tập 9.
- Các tập 10, 11, 13–20 vốn 0 caption nên không bị ảnh hưởng (luật mới chỉ loại bỏ box).

## [0.2.0] — 2026-11-14

### Thêm

- `CaptionExtractorConfig::min_duration_sec`: thời lượng tối thiểu (giây) của một cụm caption, mặc định `1.0`.
- Chế độ chạy hàng loạt cho caption trong `examples/extract_captions.rs`: không truyền tham số (hoặc `-- all`) sẽ quét mọi video trong `input/`, ghi `output/<tên>.captions.zh.vtt` và bỏ qua tập đã có VTT hợp lệ (checkpoint/resume như `src/main.rs`).
- Tham số chọn tập cho `examples/extract_captions.rs` và `examples/extract_subtitles.rs` (`-- 002`).
- Đọc caption chữ dọc: gom cột theo bề rộng ký tự, đọc trên → dưới trong từng cột rồi cột trái → cột phải, ghép `-` giữa các cột (`《大女儿-陆紫萱》`).
- Lọc chữ nằm trong cảnh cho caption: `shape_matches_text` (hình học box khớp số ký tự), `ink_looks_like_caption` (độ bão hoà màu mực) và ngưỡng `min_confidence`.
- Thử lại nhận diện trên ảnh đảo màu (`retry_vertical_missing_chars`) khi box dọc đọc thiếu ký tự.

### Thay đổi

- `CaptionRoi` mặc định `x_max` từ `0.50` lên `1.00` (ROI full width), giữ `y_min = 0.10`, `y_max = 0.80`.
- `CaptionExtractorConfig::min_confidence` mặc định từ `0.30` lên `0.80`.
- Cụm caption phải tồn tại ít nhất `min_duration_sec` (mặc định `1.0s`) mới được giữ lại.
- `select_best_caption` gom các biến thể chỉ khác dấu nối cột và ưu tiên bản có dấu nối, kết quả ổn định giữa các lần chạy.

### Sửa

- Caption dọc bị gộp hai cột liền kề khi ROI mở full width (`column_x_tolerance` tính theo bề rộng ROI) — nay dung sai co theo bề rộng ký tự.
- Ký tự hiếm bị CTC bỏ qua khi đọc cả cột dọc (`《何》` → `《何訇》`).
- Caption canh giữa bị cắt ký tự cuối do ROI chỉ phủ 50% bề rộng (`《系统》`, `《直到徐清莹突破大帝这天》`).
- Chữ trong cảnh (chữ đỏ trên tường, chữ vàng trên sách/cuộn) bị nhận nhầm thành caption.

### Gỡ bỏ

- Không có.

### Bảo mật

- Không có.

### Thay đổi phá vỡ tương thích

- `CaptionExtractorConfig` có thêm field `min_duration_sec`: code khởi tạo bằng struct literal phải bổ sung field này (hoặc dùng `..Default::default()`).
- Giá trị mặc định của `CaptionRoi::x_max` và `CaptionExtractorConfig::min_confidence` thay đổi nên kết quả caption khác so với `0.1.0`.

## [0.1.0] — 2026-11-05

### Thêm

- `extract_hardcoded_subtitles` cùng `OcrExtractorConfig`, `SubtitleRoi`, `SubtitleSegment`: trích xuất phụ đề cứng bằng DBNet + PP-OCR (ONNX), máy trạng thái thời gian và xuất WebVTT.
- `extract_captions` cùng `CaptionExtractorConfig`, `CaptionRoi`, `CaptionSegment`: trích xuất chú thích (caption) kèm lọc watermark theo tần suất xuất hiện.
- CLI hàng loạt `src/main.rs` (checkpoint/resume) và các example `extract_subtitles`, `extract_captions`.
- Tích hợp `cli_gui_form::TaskWidget` cùng callback `on_progress`, `on_step`, `on_log`.
