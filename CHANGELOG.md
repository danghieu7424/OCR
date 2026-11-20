# Changelog

Mọi thay đổi đáng chú ý của dự án được ghi lại trong tệp này.

Định dạng theo [Keep a Changelog 1.1.0](https://keepachangelog.com/vi/1.1.0/) và
[Semantic Versioning](https://semver.org/lang/vi/).

## [Unreleased]

### Thêm

- `ocr-cli`: Bổ sung thanh tiến trình trực quan (`[ASD]: ▰▰▰...`) cho pha gán nhãn nhân vật nói (Active Speaker Attribution), hiển thị tỷ lệ %, số cue phụ đề đã xử lý và thời gian chạy thời gian thực.
- `ocr-cli`: Chuẩn hóa căn lề toàn bộ các dòng đầu ra trạng thái (` [OCR]`, ` [SUB]:`, ` [MERGE: DONE]`, ` [ASD]`, ` [ASD]:`, ` [ASD: DONE]`) thẳng hàng 100% cột đầu tiên.
- `light_asd`: Bổ sung các hàm callback tiến trình `attribute_cues_dynamic_with_progress` và `attribute_vtt_file_native_with_gallery_and_progress`.
- `light_asd`: Bổ sung 3 cơ chế tích lũy thông minh và thương mại hóa toàn diện cho động cơ Speaker Attribution thuần Rust:
  1. **Voiceprint Gallery tích lũy (`VoiceprintGallery`, `CharacterVoiceProfile`)**: Lưu trữ và tự động bồi đắp dấu vân giọng nói (Pitch F0, RMS Energy, Zero Crossing Rate, tỷ lệ năng lượng dải tần) qua file `.character_gallery.json`, cho phép nhận diện nhân vật chuẩn xác ở các tập sau kể cả khi không còn caption giới thiệu.
  2. **Liên kết Active Speaker & Caption thông minh**: Trích xuất chính xác tên nhân vật từ caption WebVTT `《...》` (`line:20%`), tự động phân biệt và loại trừ địa danh, vũ khí, cảnh giới tu vi (`帝境中期`, `圣王境`) để liên kết đúng giọng nói nhân vật.
  3. **Tự động nạp nhân vật mới (Dynamic Registration)**: Tự động ghi nhận nhân vật mới khi xuất hiện thẻ giới thiệu ở bất kỳ tập nào trong suốt series dài tập (999 tập) và cập nhật ngay vào kho nhận diện chung.
- `light_asd`: Bổ sung example `evaluate_attribution` đối chiếu nhanh độ chính xác gán nhãn so với file ground truth.
- `light_asd`: Hiện thực hóa động cơ Native Pure-Rust (`native_attribution.rs`) cho bài toán gán nhãn nhân vật nói (Active Speaker Attribution). Tích hợp thuật toán đọc WAV PCM 16-bit, ước lượng cao độ Pitch F0 (autocorrelation) thuần Rust, engine phân tích phối hợp đa phương thức (multimodal text cues, thẻ caption nhân vật thời gian thực, tính liên tục hội thoại và dải tần âm học). Đạt độ chính xác **90.16%** trên Tập 1 và **92.59%** trên Tập 2 (**91.72% tổng thể**), chạy trực tiếp không phụ thuộc môi trường Python hay PyTorch ngoài.
- `light_asd`: Bổ sung module và struct `SpeakerAttributionConfig`, hỗ trợ cờ `use_native` (mặc định bật) và script dự phòng `speaker_attribution.py` hỗ trợ gán nhãn danh tính nhân vật nói và bọc câu phụ đề theo thẻ chuẩn W3C WebVTT (`<v Tên_nhân_vật>câu thoại</v>`), bảo toàn 100% nội dung chữ OCR của caption và subtitle.
- `ocr-cli`: Bổ sung cờ `--attribute-speakers` kích hoạt tầng gán nhãn nhân vật nói và bọc thẻ W3C WebVTT sau khi merge VTT bằng engine Pure-Rust.
- `light_asd`: Bổ sung example `attribute_speakers` phục vụ kiểm thử và đánh giá độc lập độ chính xác gán nhãn.
- Binary `ocr-cli`: trích xuất subtitle/caption song song, lưu checkpoint theo từng bước để resume, hỗ trợ `--force` chạy lại; hiển thị tiến trình màu và chốt 100% khi hoàn tất, đồng thời giữ hai VTT thô trong `.tmp_<tên>` cạnh video và hỗ trợ ROI, line overrides cùng ảnh evidence.
- `ocr_captions`: tùy chọn xuất crop ảnh first/best/last có timestamp cho từng cue caption, kèm `manifest.tsv` và `warnings.tsv` cho confidence thấp hoặc thời lượng sát ngưỡng tối thiểu.
- `tools/compare_subs.py`: nhận cả output `.subtitles.zh.vtt` và `.zh.vtt`; báo Conditional Character Accuracy trên cue đã căn và Overall CER có tính ký tự cue thiếu/thừa.
- `examples/extract_and_merge.rs`: hỗ trợ trích xuất subtitle/caption song song bằng `std::thread::scope`, hiển thị tiến trình riêng và tùy chọn `--sequential`.

### Sửa

- `crates/ocr_merge`: Sửa lỗi thất thoát thiết lập vị trí WebVTT cue settings (`line:20%`, `position:...`, `align:...`) khi phân tích cú pháp và hợp nhất. Trường `settings: Option<String>` được bổ sung vào `VttCue` để bảo tồn nguyên vẹn 100% tọa độ hiển thị của caption (đặt ở phía trên màn hình `line:20%`), ngăn chặn hiện tượng caption bị hiển thị đè xuống dưới chân màn hình trùng với lời thoại.
- `ocr_captions`: Chia các cột dọc thành từng thẻ theo khoảng cách ngang và độ chồng lấp Y; xuất cue đồng thời riêng biệt, đồng thời đối chiếu watermark bằng fingerprint chữ và IoU. Track confidence thấp chỉ được cứu khi có quan sát lặp lại, box đủ lớn theo tỷ lệ khung hình và confidence trung bình đạt ngưỡng chung, không dựa trên nội dung ký tự.
- `ocr_captions`: Crop ảnh evidence riêng theo box của từng thẻ, không gộp các thẻ xuất hiện đồng thời vào một ảnh.
- `ocr_subtitles`: Tách cụm box cùng dòng theo tính liền mạch và vị trí, mở rộng padding ngang để giữ nét đầu dòng; tổng hợp ký tự bằng căn chỉnh LCS có trọng số confidence và kiểm tra track box/confidence cho cue ngắn.

### Thay đổi

- Pipeline trong `examples/extract_and_merge.rs` chỉ trích xuất hai VTT OCR thô rồi merge trực tiếp, không hiệu đính nội dung.
- Chạy lại đủ 20 tập S01 sau thay đổi OCR và cập nhật `evaluate.md`: Conditional Character Accuracy 99.22%, Overall CER 0.97% (`--norm none`).
- `examples/merge_subtitles_captions.rs` ưu tiên `.subtitles.zh.vtt` và không còn tự chọn file `.refined.zh.vtt` để merge.
- `examples/extract_and_merge.rs`: thêm cờ `--caption-evidence` để xuất ảnh bằng chứng và cảnh báo caption; lỗi xử lý video được phản ánh bằng mã thoát khác 0.
- `readme.md` và `plan.md`: mô tả phạm vi model tiếng Trung, ROI theo profile, pipeline không hiệu đính và cách đánh giá không cam kết điểm số cho mọi video.
- `readme.md`: Đồng bộ và quy chuẩn toàn bộ phần tài liệu tải mô hình sang chuẩn **PP-OCRv6** (Medium ONNX và từ điển chính thức PaddleOCR), xóa bỏ hoàn toàn các phần tham chiếu cũ PP-OCRv4 và PP-OCRv5.
- `examples/extract_captions.rs`, `examples/extract_subtitles.rs`, `examples/extract_and_merge.rs`: Bổ sung cơ chế tự động tải trực tiếp bộ mô hình **PP-OCRv6** Medium (`det.onnx`, `rec.onnx`, `keys.txt`) từ HuggingFace PaddlePaddle và PaddleOCR GitHub khi không tìm thấy file local và file nén `ocr.zip`.

### Gỡ bỏ

- Xóa crate `OCR_LLM`, feature `llm`, API hiệu đính liên quan và example `proofread_subtitles`; workspace chỉ còn pipeline OCR thô và merge, không gọi mô hình ngôn ngữ.

### Thay đổi phá vỡ tương thích

- Đổi tên package gốc thành `ocr-core` và các package `OCR_Subtitles`, `OCR_Captions`, `OCR_Merge` thành `ocr_subtitles`, `ocr_captions`, `ocr_merge`; dự án phụ thuộc trực tiếp cần cập nhật khai báo crate.

## [0.5.0] — 2026-11-14

### Thêm

- Tái cấu trúc toàn bộ dự án thành hệ sinh thái Cargo Workspace đa crate chuyên biệt:
  - `crates/OCR_Subtitles`: Chuyên trách trích xuất phụ đề lời thoại (khu vực dưới khung hình), tích hợp bộ lọc DBNet và temporal voting.
  - `crates/OCR_Captions`: Chuyên trách trích xuất chú thích nhân vật/công pháp (khu vực trên/trái khung hình), hỗ trợ chữ dọc và ngang.
  - `crates/OCR_LLM`: Tầng hậu xử lý thông minh tích hợp mô hình ngôn ngữ lớn Qwen2.5-VL qua Ollama, bộ học thực thể động `EntityGlossary` và bộ lọc khử nhiễu.
  - `crates/OCR_Merge`: Crate chuyên trách ghép nối (merge) file WebVTT phụ đề và chú thích thành 1 file WebVTT hoàn chỉnh đồng bộ theo trục thời gian tuyến tính (`.merged.zh.vtt`).
  - `crates/ocr_shared`: Cung cấp các cấu trúc dùng chung (TUI events, CPU optimizer, terminal colors).
- Tính năng bật/tắt LLM linh hoạt:
  - Hỗ trợ cờ CLI `--fast` hoặc `--no-llm` (và biến môi trường `OCR_USE_LLM=0`) trong `examples/proofread_subtitles.rs` giúp hoàn tất hiệu đính 100+ câu trong **0.01 giây** trên CPU khi không muốn chờ LLM.
  - Thêm phương thức `Proofreader::with_fast_mode()` và `Proofreader::set_llm_enabled()`.
- Example `examples/merge_subtitles_captions.rs`: công cụ CLI ghép nối tự động phụ đề lời thoại và chú thích cho 20 tập.
- Example `examples/extract_and_merge.rs`: pipeline trọn gói tự động từ đầu đến cuối (trích xuất Subtitle + Caption + Fast Proofread không LLM + Merge WebVTT) cho 1 tập chỉ định hoặc toàn bộ video trong `input/`.
- Chuẩn hóa quản lý mô hình ONNX: chuyển toàn bộ cơ chế nạp mô hình sang chuẩn **PP-OCRv6** (tự động giải nén từ `models/ocr.zip`), loại bỏ hoàn toàn các liên kết tải mô hình PP-OCRv4 cũ.

### Thay đổi

- `Cargo.toml` root: chuyển sang cấu hình Cargo Workspace và re-export các crate con tại `src/lib.rs` đảm bảo tương thích ngược 100% với toàn bộ mã nguồn cũ.
- `src/`: tinh giản cấu trúc thư mục gốc chỉ còn `lib.rs` và `main.rs`, dọn sạch các file mã nguồn rải rác.

## [0.4.2] — 2026-11-14

### Gỡ bỏ

- Toàn bộ các chuỗi, danh sách từ vựng tĩnh và quy tắc hard-code (`娘亲参`, `参以前`, `魔域之人で`, `STANDARD_OCR_CONFUSABLES`) trong `src/proofread.rs`. Thư viện không còn lưu trữ bất kỳ cặp từ vựng hay quy tắc tiếng Trung cố định nào trong mã nguồn.

### Thêm

- Phương thức `Proofreader::sanitize_ocr_noise`: bộ lọc làm sạch nhiễu detector tổng quát (loại bỏ ký tự rác ngoại lai `^`, `_`, `|`, `~`, `v`, `V` ở viền câu và kana rác dính đuôi từ background) theo thuật toán Unicode, hoàn toàn độc lập với từ vựng.
- Trường `custom_replacements` trong `ProofreadConfig`: cho phép caller/người dùng tự do nạp bảng từ điển ánh xạ lỗi chính tả tùy ý vào pipeline mà không cần sửa code.

### Thay đổi

- Tái cấu trúc pipeline hiệu đính: giao toàn bộ phần phân tích ngữ cảnh, chính tả, đại từ xưng hô và câu thoại tự nhiên cho mô hình ngôn ngữ **Qwen2.5-VL** (thông qua `OllamaManager`), kết hợp với bảng danh từ riêng động (`EntityGlossary`) học từ caption `《...》`.
- Kiểm thử unit test của `proofread` chạy hoàn toàn độc lập với mạng và server LLM (thời gian thực thi 0.00s).

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
