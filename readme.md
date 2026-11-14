# ocr-core - Hardcoded Subtitle and Caption Extractor

Thư viện Rust dùng DBNet và PP-OCRv6 ONNX để phát hiện, trích xuất phụ đề cứng và caption trong video sang WebVTT. Pipeline chỉ xuất văn bản OCR thô rồi merge, không tự hiệu đính nội dung.

Model PP-OCRv6 và từ điển mặc định được tối ưu cho chữ cứng tiếng Trung tương đối sắc nét. Chất lượng thay đổi theo độ phân giải, font, hiệu ứng, vị trí chữ và model/dictionary; kết quả cần được đối chiếu với video đầu vào.

## Phạm vi và giới hạn

- `ocr_subtitles` dùng model PP-OCRv6 và mặc định quét vùng gần đáy khung hình (Y 85–100%, X 5–95%). Cần đổi profile ROI nếu subtitle nằm ở vị trí khác.
- `ocr_captions` mặc định quét vùng Y 10–80%, X 0–100%, phù hợp với profile caption của bộ dữ liệu hiện tại. ROI rộng cũng có thể chứa watermark hoặc chữ trong cảnh.
- Model và từ điển hiện tại nhắm đến tiếng Trung; không cam kết hỗ trợ mọi ngôn ngữ, font hoặc video.
- Pipeline giữ nguyên VTT OCR thô, không tự sửa nội dung. Merge chỉ sắp xếp và kết hợp cue WebVTT.
- Ảnh `--caption-evidence` hiện đã được tạo và duyệt thực tế trên 2/20 tập (001–002); chưa có xác nhận hình ảnh cho các tập còn lại.
- Không có cam kết tỷ lệ chính xác chung cho mọi video. Hãy đánh giá trên VTT tham chiếu đã kiểm chứng và xem các cue có cảnh báo bằng ảnh.

## Kiến trúc workspace

| Crate | Trách nhiệm |
|---|---|
| `ocr-core` (`lib: ocr`) | API công khai, CLI và điều phối các crate |
| `ocr_subtitles` | OCR subtitle; DBNet/PP-OCRv6, vùng mặc định Y 85–100% |
| `ocr_captions` | OCR caption; vùng mặc định Y 10–80%, xuất ảnh evidence tùy chọn |
| `ocr_merge` | Đọc, sắp xếp, merge và serialize WebVTT, bảo toàn cue settings |
| `ocr_shared` | Kiểu dữ liệu và tiện ích dùng chung |

## Pipeline OCR thô

Trích xuất subtitle và caption song song, sau đó merge trực tiếp hai VTT. Không có bước sửa nội dung OCR:

```powershell
cargo run --release --example extract_and_merge -- 001
```

Kết quả nằm trong `output/`: `.subtitles.zh.vtt`, `.captions.zh.vtt` và `.merged.zh.vtt`. Để xuất crop ảnh bằng chứng cho cue caption:

```powershell
cargo run --release --example extract_and_merge -- 001 --caption-evidence
```

Ảnh first/best/last cùng timestamp, `manifest.tsv` và `warnings.tsv` được lưu dưới `output/caption_evidence/<episode>/run-<id>/`. Ngưỡng cảnh báo confidence và thời lượng là chỉ dấu để duyệt, không phải xác suất hiệu chỉnh.

## CLI độc lập

Binary `ocr-cli` nhận một video hoặc quét không đệ quy một folder video. Mỗi file tạo một thư mục `.tmp_<tên-video>` cạnh video, luôn giữ `subtitles.zh.vtt` và `captions.zh.vtt` ở đó; file merge được ghi vào folder chỉ định:

```powershell
cargo build --release --bin ocr-cli
cargo run --release --bin ocr-cli -- input\第001集.mp4 --output output
cargo run --release --bin ocr-cli -- input --output output --caption-evidence
```

`--caption-evidence` xuất ảnh first/best/last cho cue subtitle (crop ROI subtitle) và caption (crop detector caption), kèm manifest cho từng loại. Không bật cờ thì không tạo ảnh evidence. Các tùy chọn sau điều chỉnh riêng profile trích xuất và vị trí hiển thị trong file merge; ROI dùng thứ tự `Y_MIN,Y_MAX,X_MIN,X_MAX`, giá trị tỷ lệ từ `0` đến `1`:

```powershell
cargo run --release --bin ocr-cli -- input --output output `
  --subtitle-roi 0.85,1.0,0.05,0.95 --caption-roi 0.10,0.80,0,1 `
  --subtitle-line 85% --caption-line 20%
```

`--subtitle-line` và `--caption-line` chỉ ghi đè WebVTT `line` khi merge, giữ nguyên các cue settings khác như `position` và `align`; raw VTT trong `.tmp_<tên-video>` không bị sửa. Xem `ocr-cli --help` để biết các cờ và cú pháp đầy đủ.

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
3. **MSVC Build Tools trên Windows**: Cài workload C++ của Visual Studio/Build Tools. Mở `cmd.exe`, nạp môi trường compiler trước khi chạy Cargo (giữ các lệnh trong cùng cửa sổ):
   ```cmd
   call "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat"
   ```
   Nếu cài Visual Studio ở vị trí/edition khác, thay bằng đường dẫn `vcvars64.bat` tương ứng.
4. **Mô hình ONNX**: Đặt trong thư mục `models/ocr/`:
   - `det.onnx`: DBNet Text Detection.
   - `rec.onnx`: PP-OCRv6 Text Recognition.
   - `keys.txt`: Từ điển ký tự tương ứng.

## Kiểm thử và đối chiếu

Chạy các lệnh sau từ thư mục gốc. Trên Windows, gọi chúng trong `cmd.exe` sau khi nạp `vcvars64.bat` như hướng dẫn trên:

```cmd
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
python -m unittest tools.test_compare_subs
python tools\compare_subs.py
```

`compare_subs.py` mặc định so nguyên văn (`--norm none`); Conditional Character Accuracy tính trên cue đã căn, còn Overall CER tính cả ký tự thuộc cue thiếu/thừa.

---

## 🚀 Hướng dẫn sử dụng

### 1. Dùng như Thư viện Rust (`--lib`)

Thêm `ocr-core` vào `Cargo.toml` của dự án bạn:
```toml
[dependencies]
ocr-core = { path = "../OCR" }
```

Mã nguồn trích xuất phụ đề:
```rust
use std::path::Path;
use anyhow::Result;
use ocr::{extract_hardcoded_subtitles, OcrExtractorConfig};

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
ocr::attach_cli_gui_task(Arc::clone(&task_widget));
```

#### Cách B: Lắng nghe qua Callback tùy biến
```rust
// Nhận sự kiện cập nhật tiến trình khung hình (progress)
ocr::on_progress(|p| {
    println!(
        "[{}] {}/{} {} (Thời gian: {}, Hoàn thành: {})",
        p.title, p.current, p.total, p.unit, p.elapsed, p.detail
    );
});

// Nhận sự kiện chuyển bước (step)
ocr::on_step(|step| {
    println!(">>> {}", step);
});

// Nhận log nội bộ engine
ocr::on_log(|log| {
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