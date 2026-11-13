# Changelog

Mọi thay đổi đáng chú ý của dự án được ghi lại trong tệp này.

Định dạng theo [Keep a Changelog 1.1.0](https://keepachangelog.com/vi/1.1.0/) và
[Semantic Versioning](https://semver.org/lang/vi/).

## [Unreleased]

Chưa có thay đổi.

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
