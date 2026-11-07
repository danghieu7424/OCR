# OCR - Industrial-Grade Hardcoded Subtitle Extractor

Thư viện và công cụ Rust chuẩn công nghiệp dùng để phát hiện và trích xuất phụ đề cứng (Hardcoded Subtitles) từ video sang định dạng WebVTT (`.zh.vtt`).

Được xây dựng trên nền tảng **ONNX Runtime**, **DBNet Text Detection**, **PP-OCRv4/v5 CTC Recognition**, kết hợp máy trạng thái thời gian (**Temporal State Machine**) và thuật toán đối sánh chuỗi thông minh (Levenshtein + Prefix/Suffix Sequence Matching) đạt độ chính xác >99%:
- **Bảo toàn 100% Sub 1 ký tự**: Nhận diện chuẩn xác các từ đơn lẻ ngắn (`爹`, `嗯`, `姐`, `哼`, `啊`).
- **Chống gộp cụm & rụng câu**: Phân tách chuẩn xác các câu thoại nói nhanh liền kề.
- **Khử nhiễu nhấp nháy**: Triệt tiêu hiện tượng chữ nháy nét, vệt sáng kiếm quang hoặc typo 1-frame.
- **Tự động bù thời gian**: Tinh chỉnh mốc thời gian bắt đầu (`-50ms`) đồng bộ tuyệt đối với khẩu hình và âm thanh.

---

## 📦 Yêu cầu môi trường

1. **FFmpeg & FFprobe**: Cần được cài đặt sẵn và thêm vào `PATH` hệ thống.
2. **Rust Toolchain**: Rust 2024 Edition.
3. **Mô hình AI ONNX**: Đặt trong thư mục `models/ocr/`:
   - `det.onnx`: DBNet Text Detection.
   - `rec.onnx`: PP-OCRv4 hoặc PP-OCRv5 Text Recognition.
   - `keys.txt`: Từ điển ký tự tương ứng.

---

## 🚀 Hướng dẫn sử dụng

### 1. Dùng như Thư viện Rust (`--lib`)

Thêm `OCR` vào `Cargo.toml` của dự án bạn:
```toml
[dependencies]
OCR = { path = "../OCR" }
```

Mã nguồn trích xuất phụ đề:
```rust
use std::path::Path;
use anyhow::Result;
use OCR::{extract_hardcoded_subtitles, OcrExtractorConfig};

fn main() -> Result<()> {
    // 1. Cấu hình thư mục chứa mô hình (chứa thư mục con `ocr/` gồm det.onnx, rec.onnx, keys.txt)
    let model = Path::new("models/ocr/rec.onnx");
    let (model_dir, _engine) = if model.exists() {
        (Path::new("models"), "local_onnx")
    } else {
        (Path::new("."), "root_fallback")
    };

    // 2. Cấu hình trích xuất (đã tinh chỉnh mặc định tối ưu)
    let mut config = OcrExtractorConfig::default();
    config.fps = 10;                // Tốc độ quét 10 fps
    config.accuracy_mode = true;    // Chế độ nhận diện chính xác cao
    // config.roi.y_min = 0.85;     // Mặc định: 85% - 100% sát đáy, tránh vùng sub chú thích
    // config.roi.y_max = 1.00;

    // 3. Đường dẫn video đầu vào và file vtt đầu ra
    let video_path = Path::new("video_input/sample.mp4");
    let output_vtt = Path::new("output/sample.zh.vtt");

    // 4. Khởi chạy trích xuất
    let result = extract_hardcoded_subtitles(video_path, output_vtt, model_dir, &config)?;

    println!(">>> Hoàn thành! Đã trích xuất {} câu phụ đề vào {:?}", result.segments.len(), result.vtt_path);
    Ok(())
}
```

---

### 2. Tích hợp thanh tiến trình với `cli_gui_form` (TUI Ready)

Thư viện hỗ trợ sẵn cơ chế cắm trực tiếp vào `TaskWidget` của `cli_gui_form` hoặc lắng nghe qua callback:

#### Cách A: Cắm trực tiếp vào `cli_gui_form::TaskWidget`
```rust
use std::sync::{Arc, Mutex};
use cli_gui_form::TaskWidget;
use ratatui::style::Color;

let task_widget = Arc::new(Mutex::new(TaskWidget::new_loading(
    "ocr_task",
    "OCR Video Subtitles",
    "Đang chuẩn bị mô hình...",
    Color::Cyan,
)));

// Gắn widget trực tiếp vào hệ thống sự kiện tiến trình của OCR
OCR::attach_cli_gui_task(Arc::clone(&task_widget));
```

#### Cách B: Lắng nghe qua Callback tùy biến
```rust
// Nhận sự kiện cập nhật tiến trình khung hình (progress)
OCR::on_progress(|p| {
    println!(
        "[{}] {}/{} {} (Thời gian: {}, Hoàn thành: {})",
        p.title, p.current, p.total, p.unit, p.elapsed, p.detail
    );
});

// Nhận sự kiện chuyển bước (step)
OCR::on_step(|step| {
    println!(">>> {}", step);
});

// Nhận log nội bộ engine
OCR::on_log(|log| {
    println!("    {}", log);
});
```

---

### 3. Chạy file ví dụ mẫu (`examples/`)

Dự án cung cấp sẵn ví dụ hoàn chỉnh trong thư mục `examples/`:
```bash
cargo run --example extract_subtitles
```

Ví dụ này minh họa toàn bộ quy trình:
- Tự động kiểm tra file model tồn tại để cấu hình đường dẫn.
- Khởi tạo và cắm `TaskWidget` từ `cli_gui_form`.
- Đăng ký nhận sự kiện tiến trình và gọi trích xuất video sang WebVTT.

---

### 4. Chạy chế độ hàng loạt CLI (`--bin`)

Binary tích hợp sẵn cơ chế **Checkpoint & Resume** tự động quét hàng loạt video:
```bash
cargo run --release
```

- **Tự động quét**: Đọc tất cả video `.mp4`, `.mkv`, `.avi` trong thư mục input và sắp xếp theo thứ tự số tập (`第001集`, `第002集`,...).
- **Tự động Resume**: Kiểm tra thư mục `output/`, nếu các tập trước đã có file `.zh.vtt` hợp lệ thì tự động bỏ qua và tiếp tục chạy từ tập chưa hoàn thành.
- **Xuất nhật ký kép**: Ghi đồng thời toàn bộ tiến trình ra console và append vào file [`logs/1.log`](logs/1.log).

---

## 📥 Tải mô hình và từ điển ký tự

Hệ thống OCR yêu cầu trọn bộ 3 tệp mô hình đặt tại thư mục `models/ocr/` (chương trình sẽ tự động tải về nếu phát hiện thiếu):

### 🔍 1. Tải mô hình phát hiện vị trí chữ (`det.onnx`)

Mô hình DBNet dùng để quét toàn khung hình và định vị tọa độ (bounding box) của từng dòng phụ đề:
- **Tên mô hình chuẩn:** `ch_PP-OCRv4_det_infer.onnx` (đổi tên thành `det.onnx`)
- **Link tải:** [https://huggingface.co/SWHL/RapidOCR/resolve/main/PP-OCRv4/ch_PP-OCRv4_det_infer.onnx](https://huggingface.co/SWHL/RapidOCR/resolve/main/PP-OCRv4/ch_PP-OCRv4_det_infer.onnx)
- **Lệnh tải nhanh:**
  ```bash
  curl.exe -L -o models/ocr/det.onnx https://huggingface.co/SWHL/RapidOCR/resolve/main/PP-OCRv4/ch_PP-OCRv4_det_infer.onnx
  ```

---

### 🔤 2. Từ điển ký tự (`keys.txt`)

Model OCR CTC yêu cầu file từ điển tương thích đúng số lớp ký tự (18,384 dòng ký tự + blank index 0 = 18,385 class):
- **File từ điển chuẩn PP-OCRv5:** `ppocrv5_dict.txt` (đổi tên thành `keys.txt`)
- **Link tải:** [https://github.com/GreatV/oar-ocr/releases/download/v0.1.0/ppocrv5_dict.txt](https://github.com/GreatV/oar-ocr/releases/download/v0.1.0/ppocrv5_dict.txt)
- **Lệnh tải nhanh:**
  ```bash
  curl.exe -L -o models/ocr/keys.txt https://github.com/GreatV/oar-ocr/releases/download/v0.1.0/ppocrv5_dict.txt
  ```

---

### 📥 3. Tải mô hình nhận diện chữ (`rec.onnx`)

Mô hình PP-OCR CTC dùng để dịch các mẩu ảnh chữ thành văn bản tiếng Trung:
- **Bản PP-OCRv5 Server Rec ONNX:** `ppocrv5_server_rec.onnx` (đổi tên thành `rec.onnx`)
- **Link tải:** [https://github.com/GreatV/oar-ocr/releases/download/v0.1.0/ppocrv5_server_rec.onnx](https://github.com/GreatV/oar-ocr/releases/download/v0.1.0/ppocrv5_server_rec.onnx)
- **Lệnh tải nhanh:**
  ```bash
  curl.exe -L -o models/ocr/rec.onnx https://github.com/GreatV/oar-ocr/releases/download/v0.1.0/ppocrv5_server_rec.onnx
  ```
