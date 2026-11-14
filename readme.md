# OCR - Hardcoded Subtitle and Caption Extractor

Thư viện Rust dùng DBNet và PP-OCRv6 ONNX để phát hiện, trích xuất phụ đề cứng và caption trong video sang WebVTT. Pipeline mặc định giữ nguyên văn bản OCR thô, không hiệu đính nội dung.

Mặc định hiện tại được tối ưu cho video tiếng Trung có chữ cứng tương đối sắc nét. Chất lượng thay đổi theo độ phân giải, font, hiệu ứng, vị trí chữ và model/dictionary; kết quả cần được đối chiếu với video đầu vào.

## Phạm vi và giới hạn

- `OCR_Subtitles` dùng model PP-OCRv6 và mặc định quét vùng gần đáy khung hình (Y 85–100%, X 5–95%). Cần đổi ROI nếu subtitle nằm ở vị trí khác.
- `OCR_Captions` mặc định quét vùng Y 10–80%, X 0–100%, phù hợp với profile caption của bộ dữ liệu hiện tại. ROI rộng cũng có thể chứa watermark hoặc chữ trong cảnh.
- Model và từ điển hiện tại nhắm đến tiếng Trung; không cam kết hỗ trợ mọi ngôn ngữ, font hoặc video.
- Không có cam kết tỷ lệ chính xác chung cho mọi video. Hãy đánh giá trên VTT tham chiếu đã kiểm chứng và xem các cue có cảnh báo bằng ảnh.

## Pipeline OCR thô

Trích xuất subtitle và caption song song, sau đó merge trực tiếp hai VTT. Nội dung OCR không bị sửa bởi LLM, glossary hoặc fuzzy matching:

```powershell
cargo run --release --example extract_and_merge -- 001
```

Kết quả nằm trong `output/`: `.subtitles.zh.vtt`, `.captions.zh.vtt` và `.merged.zh.vtt`. Để xuất crop ảnh bằng chứng cho cue caption:

```powershell
cargo run --release --example extract_and_merge -- 001 --caption-evidence
```

Ảnh first/best/last cùng timestamp, `manifest.tsv` và `warnings.tsv` được lưu dưới `output/caption_evidence/<episode>/run-<id>/`. Ngưỡng cảnh báo confidence và thời lượng là chỉ dấu để duyệt, không phải xác suất hiệu chỉnh.

## Đối chiếu VTT

Script đọc được cả output mới `.subtitles.zh.vtt` và tên cũ `.zh.vtt`. Mặc định không cần cài gói chuẩn hóa:

```powershell
python tools\compare_subs.py
```

`Conditional Character Accuracy` chỉ tính các cặp cue đã căn; `Overall CER` tính thêm ký tự của cue thiếu và thừa. Tùy chọn `--norm t2s` dùng để so sánh phồn thể/giản thể, cần cài `zhconv` riêng và không sửa file trong `input/`.

---

## 📦 Yêu cầu môi trường

1. **FFmpeg & FFprobe**: Cần được cài đặt sẵn và thêm vào `PATH` hệ thống.
2. **Rust Toolchain**: Rust 2024 Edition.
3. **Mô hình AI ONNX**: Đặt trong thư mục `models/ocr/`:
   - `det.onnx`: DBNet Text Detection.
   - `rec.onnx`: PP-OCRv6 Text Recognition.
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

    // 2. Cấu hình trích xuất; điều chỉnh ROI theo vị trí subtitle của video
    let mut config = OcrExtractorConfig::default();
    config.fps = 10;                // Tốc độ quét 10 fps
    config.accuracy_mode = true;    // Chế độ nhận diện chính xác cao
    // config.roi.y_min = 0.85;     // ROI mặc định của subtitle: 85% - 100%
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

## 📥 Tải mô hình và từ điển ký tự (PP-OCRv6)

Hệ thống OCR yêu cầu trọn bộ 3 tệp mô hình PP-OCRv6 đặt tại thư mục `models/ocr/` (chương trình sẽ tự động tải về hoặc giải nén từ `models/ocr.zip` nếu phát hiện thiếu):

### 🔍 1. Tải mô hình phát hiện vị trí chữ (`det.onnx`) - PP-OCRv6 Medium

Mô hình DBNet dùng để quét toàn khung hình và định vị tọa độ (bounding box) của từng dòng phụ đề:
- **Link tải:** [https://huggingface.co/PaddlePaddle/PP-OCRv6_medium_det_onnx/resolve/main/inference.onnx](https://huggingface.co/PaddlePaddle/PP-OCRv6_medium_det_onnx/resolve/main/inference.onnx)
- **Lệnh tải nhanh:**
  ```bash
  curl.exe -L -o models/ocr/det.onnx https://huggingface.co/PaddlePaddle/PP-OCRv6_medium_det_onnx/resolve/main/inference.onnx
  ```

---

### 🔤 2. Từ điển ký tự (`keys.txt`) - PP-OCRv6 Dictionary

File từ điển ký tự chính thức của PP-OCRv6:
- **Link tải:** [https://raw.githubusercontent.com/PaddlePaddle/PaddleOCR/main/ppocr/utils/dict/ppocrv6_dict.txt](https://raw.githubusercontent.com/PaddlePaddle/PaddleOCR/main/ppocr/utils/dict/ppocrv6_dict.txt)
- **Lệnh tải nhanh:**
  ```bash
  curl.exe -L -o models/ocr/keys.txt https://raw.githubusercontent.com/PaddlePaddle/PaddleOCR/main/ppocr/utils/dict/ppocrv6_dict.txt
  ```

---

### 📥 3. Tải mô hình nhận diện chữ (`rec.onnx`) - PP-OCRv6 Medium

Mô hình PP-OCRv6 CTC dùng để dịch các mẩu ảnh chữ thành văn bản tiếng Trung:
- **Link tải:** [https://huggingface.co/PaddlePaddle/PP-OCRv6_medium_rec_onnx/resolve/main/inference.onnx](https://huggingface.co/PaddlePaddle/PP-OCRv6_medium_rec_onnx/resolve/main/inference.onnx)
- **Lệnh tải nhanh:**
  ```bash
  curl.exe -L -o models/ocr/rec.onnx https://huggingface.co/PaddlePaddle/PP-OCRv6_medium_rec_onnx/resolve/main/inference.onnx
  ```