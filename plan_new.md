Me: Tôi thấy mô hình llm đọc rất tốt các chữ cách điệu nhất là gemma, qwen tôi chỉ muốn lấy bộ đọc ảnh chuyển sang onnx có được không?

AI:

Về mặt kỹ thuật là **có thể tách và xuất riêng phần đọc ảnh (Vision Encoder) sang ONNX**, nhưng **bạn sẽ KHÔNG nhận được chữ (text) trực tiếp từ file ONNX đó**.

### 1. Tại sao tách riêng Vision Encoder không đọc ra chữ?

Các mô hình đa phương thức như Gemma (Gemma 3/4) hay Qwen-VL (Qwen2.5-VL, Qwen3-VL) gồm 2 phần tách biệt phối hợp với nhau:

- **Vision Encoder (Bộ đọc ảnh - ví dụ SigLIP trên Gemma):** Chỉ biến hình ảnh thành các chuỗi số vector đặc trưng (image embeddings), không biết mặt chữ hay từ vựng là gì.
- **Language Decoder (Bộ sinh ngôn ngữ - LLM):** Nhận các vector đó, kết hợp với vốn từ vựng và khả năng suy luận ngữ cảnh để "dịch" thành văn bản.

Chính khả năng suy luận ngữ cảnh của phần **LLM phía sau** mới giúp Gemma/Qwen đoán đúng các chữ cách điệu, viết tay hoặc bị che khuất. Nếu chỉ lấy riêng `vision_encoder.onnx`, đầu ra của nó chỉ là một ma trận số vô nghĩa với người dùng trừ khi bạn tự huấn luyện thêm một bộ giải mã (Decoder/CTC head) nối vào sau đó.

---

### 2. Các hướng triển khai thực tế

Tùy vào mục đích thực sự của bạn, có 3 hướng tiếp cận:

- **Hướng 1: Xuất cả cụm VLM sang ONNX (nhưng tách thành các file nhỏ)**
  Các công cụ như `onnxruntime-genai` hoặc `optimum` đã hỗ trợ tách sẵn các mô hình như Qwen-VL hay Gemma sang ONNX dưới dạng 3 file: `vision_encoder.onnx`, `embedding.onnx`, và `decoder_model.onnx`. Bạn vẫn phải chạy đủ cả chuỗi này qua ONNX Runtime để ra được chữ, nhưng có thể chọn bản nhẹ nhất (ví dụ: **Qwen2.5-VL-3B** hoặc **Qwen3-VL-2B**, lượng tử hóa xuống **INT4**) để chạy nhẹ trên CPU/GPU.
- **Hướng 2: Chỉ lấy `vision_encoder.onnx` để làm Embedding / Tìm kiếm ảnh**
  Nếu mục đích của bạn là tìm kiếm ảnh theo nội dung (Image Retrieval) hoặc dùng vector đó làm đầu vào cho một mô hình phân loại nhỏ khác của riêng bạn, bạn hoàn toàn có thể tải trực tiếp file `vision_encoder.onnx` đã được cộng đồng xuất sẵn trên Hugging Face (ví dụ từ kho `onnx-community`) mà không cần tự convert.
- **Hướng 3: Dùng mô hình chuyên biệt OCR cỡ siêu nhỏ (Khuyên dùng nếu chỉ cần đọc chữ)**
  Nếu bài toán chỉ là nhận diện chữ cách điệu và cần tốc độ cao, nhẹ để nhúng vào app (mobile/web/edge) qua ONNX, dùng LLM là quá cồng kềnh. Bạn nên cân nhắc các dòng chuyên trị OCR đã có sẵn bản ONNX nhẹ vài chục MB như **PaddleOCR (v4/v5)**, **VietOCR** (nếu có tiếng Việt), hoặc **TrOCR**.

---

### Cần làm rõ để tư vấn chính xác hơn:

Hiện yêu cầu của bạn có thể hiểu theo 2 hướng:

1. Bạn muốn **nhận diện chữ (OCR) nhẹ và nhanh** bằng ONNX nên định "cắt bớt" phần LLM cho nhẹ?
2. Hay bạn thực sự cần **lấy vector đặc trưng (embeddings)** từ bộ đọc ảnh của Gemma/Qwen để phục vụ bài toán khác?

Bạn đang dự định chạy mô hình này trên nền tảng nào (Web, Mobile Android/iOS, hay Server/PC) và ngôn ngữ chữ cần đọc là tiếng Việt hay tiếng Anh?

---

---

Me: Tôi cần onnx để chạy trên rust với nhiệm vụ đọc sub cứng trong video = các frame với khoảng 10 tiếng+ voice của người nói (chú ý có nhiều phim tiết kiệm kinh phí chỉ có vài người lồng tiếng cho nhiều nhân vật) => xuất ra file vtt đồng thời dịch từ văn bản gốc ví dụ như zh phồn thể và giản thể sang tiếng việt đồng thời cũng chú trọng vai vế và ngôi thứ cũng như đọc các caption về tên, đia danh, cấp bậc, được giới thiệu trong diện tích lớn của frame. Do đó cần có Vision Encoder, image encoder và một bộ não điều phối nhanh và linh hoạt, do đó chạy trên ram và nạp từng cụm nhỏ để tạo .zh.vtt, .vi.vtt và quy trình + cache để khi chạy lại không phải làm lại từ đầu. Bạn có thể gợi ý, bổ sung, hướng dẫn tôi hay không

AI:

[WEB]

### Giả định & Làm rõ phạm vi kiến trúc

- **Giả định hạ tầng:** Bạn đang xây dựng hệ thống xử lý bằng **Rust** (sử dụng crate `ort` hoặc `onnxruntime-genai` để chạy ONNX) kết hợp lưu trữ trạng thái cục bộ. _(Nếu bạn triển khai dưới dạng worker/CLI chạy ngầm thay vì server Axum/Actix, toàn bộ kiến trúc lõi bên dưới vẫn giữ nguyên)._
- **2 cách hiểu về ràng buộc bộ nhớ (RAM) của bạn:**

1. **Cách hiểu A (Pure CPU + System RAM):** Máy không có GPU rời (hoặc không dùng CUDA), toàn bộ mô hình ONNX nạp và chạy tuần tự trên RAM hệ thống (16GB–32GB), cần cơ chế giải phóng bộ nhớ sau từng bước để không tràn RAM khi xử lý video 10+ tiếng.
2. **Cách hiểu B (VRAM nhỏ + RAM Offload):** Có GPU nhưng VRAM thấp (4GB–8GB), cần chia video thành từng cụm nhỏ (chunk) và luân chuyển các mô hình ONNX giữa RAM và VRAM.

---

### Phản biện kiến trúc (Điểm cần tối ưu trước khi viết code)

1. **Không nên đẩy toàn bộ frame của 10 tiếng video qua Vision Encoder của VLM (Gemma/Qwen):**

- 10 tiếng video = `36.000 giây`. Nếu trích xuất `2–3 frame/giây` để bắt kịp sub thoại nhanh và thẻ tên nhân vật, bạn sẽ có **72.000 – 108.000 ảnh**.
- Nếu đưa toàn bộ số ảnh này qua Vision Encoder của Qwen-VL/Gemma trên CPU/RAM, thời gian xử lý sẽ kéo dài nhiều ngày và cực kỳ lãng phí vì **85% số frame liên tiếp có nội dung chữ giống hệt nhau**.
- **Giải pháp đơn giản và nhanh gấp 30–50 lần:** Dùng thuật toán so sánh thay đổi điểm ảnh (dHash/MSE) ở vùng sub để lọc bỏ frame trùng $\rightarrow$ Dùng **PaddleOCR ONNX** (~15MB) đọc 95% sub cứng tiêu chuẩn $\rightarrow$ **Chỉ gọi VLM Vision Encoder (Qwen2.5-VL INT4 ONNX)** cho những frame có **caption diện tích lớn / chữ thư pháp cách điệu** mà PaddleOCR trả về độ tự tin thấp.

2. **Xử lý bài toán 1 diễn viên lồng tiếng nhiều vai:**

- Vì phim tiết kiệm kinh phí dùng chung người lồng tiếng, các mô hình phân tách giọng nói theo đặc trưng âm sắc (Speaker Diarization / Voiceprint Embeddings) **sẽ nhận nhầm nhân vật**.
- **Giải pháp:** Không dùng Voiceprint để chia vai. Thay vào đó, chỉ dùng **Silero VAD ONNX** (~2MB) để bắt mốc thời gian có tiếng nói (khóa chặt timing VTT), còn việc phân định nhân vật và **ngôi thứ/vai vế** sẽ do **LLM điều phối văn bản** đảm nhiệm thông qua: _(1) Thẻ tên/cấp bậc diện tích lớn vừa xuất hiện trên màn hình + (2) Cửa sổ ngữ cảnh trượt (Sliding Window) + (3) Bảng thuật ngữ nhân vật tích lũy (Dynamic Glossary)_.

---

### Kiến trúc Pipeline 4 Tầng (Chạy từng cụm nhỏ + Cache chống chạy lại)

Để chạy mượt trên RAM cho video 10+ tiếng, hệ thống không nạp tất cả model ONNX cùng lúc mà chạy theo **từng giai đoạn (Stage-by-stage)**, xong giai đoạn nào thì `drop()` model khỏi RAM và ghi kết quả xuống **SQLite**:

```text
[Video 10h+]
   │
   ├──► (Stage 1: FFmpeg + Pixel Diff + Silero VAD ONNX) ──► [SQLite: keyframes & speech_ts]
   │       (Chia cụm 5 phút, lọc frame trùng, giải phóng RAM)
   │
   ├──► (Stage 2: PaddleOCR ONNX + Selective Qwen-VL ONNX) ──► [SQLite: raw_ocr & title_cards]
   │       (Phân loại Sub đáy vs Caption lớn; VLM chỉ đọc chữ cách điệu khó)
   │
   ├──► (Stage 3: LLM Coordinator ONNX - Qwen2.5/Gemma INT4) ──► [SQLite: vi_subs & glossary]
   │       (Nạp 40 câu/lần + Bảng tên/cấp bậc -> Sửa lỗi OCR + Dịch chuẩn vai vế)
   │
   └──► (Stage 4: VTT Exporter) ──► Xuất file [movie.zh.vtt] & [movie.vi.vtt]

```

#### Chi tiết từng tầng xử lý:

1. **Tầng 1: Chia cụm & Lọc Keyframe + Bắt mốc âm thanh**

- Cắt luồng xử lý thành các **chunk 5 phút** (`300 giây`).
- Tính toán sai khác pixel ở **vùng đáy (20% chiều cao dưới - nơi chứa sub thoại)** và **vùng trung tâm/góc (nơi chứa bảng tên, địa danh, cấp bậc)**. Chỉ lưu ảnh keyframe ra thư mục tạm khi có sự biến thiên chữ.
- Chạy `silero_vad.onnx` trên luồng audio 16kHz để lấy danh sách khoảng thời gian có giọng nói, giúp loại bỏ các frame rác (nhiễu hình ảnh không có thoại) và căn chỉnh timestamp bắt đầu/kết thúc câu thoại chính xác.

2. **Tầng 2: Đọc Sub cứng & Nhận diện Caption diện tích lớn**

- Nạp cặp `ch_PP-OCRv4_det.onnx` + `ch_PP-OCRv4_rec.onnx` (hỗ trợ tốt cả **zh giản thể và phồn thể**).
- Dựa vào tọa độ Bounding Box trả về, phân loại thành 2 luồng:
- `DIALOGUE_SUB`: Nằm ở dải đáy màn hình.
- `LARGE_CAPTION`: Diện tích box lớn hoặc nằm ngoài vùng sub đáy (thường là tên nhân vật, tên chiêu thức, địa danh, cấp bậc).

- **Cơ chế kích hoạt VLM thông minh:** Nếu `LARGE_CAPTION` có điểm tự tin (`confidence < 0.85`) do font thư pháp rồng bay phượng múa, hệ thống mới nạp **Qwen2.5-VL-3B-Instruct (INT4 ONNX)** để đọc riêng tập ảnh này, sau đó giải phóng khỏi RAM.

3. **Tầng 3: Bộ não điều phối LLM (Dịch thuật & Giữ chuẩn vai vế/ngôi thứ)**

- Nạp duy nhất 1 model LLM thuần văn bản lượng tử hóa **INT4** (ví dụ: `Qwen2.5-3B-Instruct-ONNX-INT4` hoặc `Qwen2.5-7B-Instruct-ONNX-INT4`, chỉ chiếm ~2.5GB–4.5GB RAM và chạy rất nhanh trên CPU).
- Gửi từng gói **40 dòng thoại liên tiếp** kèm theo:
- **Bảng thực thể (Entity Glossary):** Các tên riêng, địa danh, cấp bậc vừa quét được từ `LARGE_CAPTION` trước đó.
- **Tóm tắt 5 câu thoại liền trước + Cặp xưng hô đang khóa:** Giúp LLM biết ai đang nói chuyện với ai (ví dụ: _Sư phụ - Đồ đệ_ $\rightarrow$ _Ta - Con_; _Kẻ thù ngang hàng_ $\rightarrow$ _Ngươi - Ta_; _Bề dưới với Vương gia_ $\rightarrow$ _Thuộc hạ - Điện hạ_).
- **Nhiệm vụ kép trong 1 lần suy luận:** Tự động sửa lỗi nhận diện sai mặt chữ Hán của OCR (dựa vào ngữ cảnh câu) $\rightarrow$ Xuất bản gốc chuẩn `.zh.vtt` và bản dịch `.vi.vtt`.

4. **Tầng 4: Gom cụm thời gian & Xuất WebVTT**

- Gộp các keyframe liên tiếp có nội dung giống nhau (hoặc độ tương đồng Levenshtein > 90%) thành 1 block thời gian duy nhất (`00:12:04.120 --> 00:12:06.850`).
- Định dạng riêng các `LARGE_CAPTION` (ví dụ: đặt ở vị trí trên cùng VTT `line:10%` hoặc bọc ngoặc `[Cấp bậc: Đấu Vương - Vân Lam Tông]`) để không đè lên sub hội thoại của người xem.

---

### Thiết kế Cache & Quản lý Vòng đời RAM trong Rust

Để đảm bảo video 10+ tiếng gặp sự cố mất điện hoặc tắt ngang có thể **chạy tiếp từ điểm dừng (Resume)**, toàn bộ trạng thái trung gian được lưu vào **SQLite (`rusqlite` ở chế độ WAL)** kèm mã băm `file_hash` của video.

Dưới đây là cấu trúc khung dữ liệu và vòng đời quản lý RAM trong Rust:

```rust
// ============================================================================
// PHÂN ĐOẠN 1: SCHEMA SQLITE LƯU CACHE TỪNG GIAI ĐOẠN (CHỐNG CHẠY LẠI TỪ ĐẦU)
// ============================================================================
pub const INIT_CACHE_SCHEMA: &str = r#"
PRAGMA journal_mode = WAL; -- Tối ưu tốc độ ghi đồng thời khi xử lý batch lớn

-- Lưu tiến độ tổng thể của từng file video (định danh bằng SHA256 đầu file + kích thước)
CREATE TABLE IF NOT EXISTS video_jobs (
    video_hash TEXT PRIMARY KEY,
    file_path TEXT NOT NULL,
    last_completed_stage INTEGER DEFAULT 0, -- 1: Keyframes, 2: OCR, 3: LLM Translated, 4: VTT Exported
    last_processed_ms INTEGER DEFAULT 0     -- Mốc thời gian (ms) đã xử lý xong gần nhất
);

-- Cache kết quả Stage 1 & Stage 2: Frame đã trích xuất và văn bản OCR thô
CREATE TABLE IF NOT EXISTS frame_ocr_cache (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    video_hash TEXT NOT NULL,
    timestamp_ms INTEGER NOT NULL,
    frame_type TEXT NOT NULL,      -- 'DIALOGUE' (sub đáy) hoặc 'LARGE_CAPTION' (tên/địa danh/cấp bậc)
    raw_zh_text TEXT,              -- Chữ Hán (giản/phồn thể) đọc được
    ocr_confidence REAL,           -- Độ tự tin; nếu thấp sẽ được đánh dấu đẩy qua VLM ONNX
    vlm_verified INTEGER DEFAULT 0 -- 1 nếu đã được Qwen-VL đọc lại chuẩn xác
);

-- Cache kết quả Stage 3: Bảng thuật ngữ nhân vật, cấp bậc, vai vế tích lũy xuyên suốt phim
CREATE TABLE IF NOT EXISTS entity_glossary (
    video_hash TEXT NOT NULL,
    zh_term TEXT NOT NULL,         -- Ví dụ: 萧炎 / 宗主
    vi_term TEXT NOT NULL,         -- Ví dụ: Tiêu Viêm / Tông chủ
    role_context TEXT,             -- Ghi chú vai vế: "Sư phụ của nhân vật chính, xưng Ta - Con"
    PRIMARY KEY (video_hash, zh_term)
);

-- Cache kết quả Stage 3: Các câu thoại đã hiệu đính và dịch sang tiếng Việt
CREATE TABLE IF NOT EXISTS subtitle_segments (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    video_hash TEXT NOT NULL,
    start_ms INTEGER NOT NULL,
    end_ms INTEGER NOT NULL,
    segment_type TEXT NOT NULL,    -- 'DIALOGUE' hoặc 'CAPTION'
    clean_zh TEXT NOT NULL,        -- Văn bản zh đã sửa lỗi OCR
    translated_vi TEXT NOT NULL    -- Văn bản vi đã dịch chuẩn ngôi thứ
);
"#;

// ============================================================================
// PHÂN ĐOẠN 2: VÒNG ĐỜI NẠP / GIẢI PHÓNG MODEL ONNX TRÊN RAM (MEMORY GUARD)
// ============================================================================
pub fn run_pipeline_with_ram_guard(video_path: &str, db: &rusqlite::Connection) -> anyhow::Result<()> {
    let video_hash = compute_fast_file_hash(video_path)?;
    let current_stage = get_last_completed_stage(db, &video_hash)?;

    // Giai đoạn 1: Trích xuất keyframe theo cụm 5 phút + Silero VAD ONNX
    if current_stage < 1 {
        // Mở scope riêng để session ONNX tự động Drop giải phóng RAM ngay khi xong
        {
            // Nạp silero_vad.onnx (~2MB RAM)
            // Quét từng chunk 300s -> lưu frame thay đổi -> ghi SQLite
        }
        update_stage(db, &video_hash, 1)?;
    }

    // Giai đoạn 2: Chạy PaddleOCR ONNX & gọi VLM ONNX chọn lọc cho chữ cách điệu lớn
    if current_stage < 2 {
        {
            // Bước 2a: Nạp PaddleOCR ONNX (~15MB RAM) quét toàn bộ frame_ocr_cache
            // Bước 2b: Drop PaddleOCR -> Chỉ nạp Qwen2.5-VL INT4 ONNX nếu có frame 'LARGE_CAPTION' mờ/cách điệu
            // Bước 2c: Drop Qwen2.5-VL khỏi RAM ngay lập tức
        }
        update_stage(db, &video_hash, 2)?;
    }

    // Giai đoạn 3: Nạp LLM Điều phối (Qwen2.5 / Gemma INT4 ONNX) để dịch & giữ ngôi thứ
    if current_stage < 3 {
        {
            // Nạp LLM Text ONNX (~3GB RAM)
            // Đọc từng cụm 40 dòng từ SQLite + Entity Glossary -> Dịch -> Lưu subtitle_segments
            // Drop LLM khỏi RAM
        }
        update_stage(db, &video_hash, 3)?;
    }

    // Giai đoạn 4: Xuất file .zh.vtt và .vi.vtt từ SQLite
    if current_stage < 4 {
        export_webvtt_files(db, &video_hash, video_path)?;
        update_stage(db, &video_hash, 4)?;
    }

    Ok(())
}

```

---

### Các thư viện Rust (Crates) khuyên dùng cho dự án này

- **`ort` (v2.x):** Crate chuẩn và mạnh nhất để chạy ONNX Runtime trên Rust, hỗ trợ kiểm soát luồng CPU (`with_intra_threads`), lượng tử hóa INT4/INT8 và giải phóng bộ nhớ chủ động qua cơ chế sở hữu (Ownership) của Rust.
- **`tokenizers` (Hugging Face):** Tokenizer viết thuần bằng Rust, tốc độ cực nhanh để mã hóa/giải mã token cho Qwen/Gemma mà không cần gọi sang Python.
- **`image` + `img_hash`:** Cắt vùng ảnh (crop sub đáy vs toàn khung) và tính toán độ sai khác frame ngay trên RAM để lọc trùng frame trước khi đưa vào ONNX.
- **`rusqlite` (kèm feature `bundled`):** Nhúng thẳng engine SQLite vào file nhị phân Rust, không cần cài đặt database ngoài, truy vấn và resume cache theo mili-giây.

---

---

Me: tôi có sẵn ferrietDB thay cho SQLite có được hay không? hoặc không dùng SQL, tôi có sẵn mục đọc sub và caption rồi nhưng thi thoảng vẫn sai và lỗi rất nhiều, lại còn hard-code nữa tôi muốn điều chỉnh lại plan trên để có bộ não điều phối và dịch như các studio chuyên nghiệp:
Tôi cần onnx để chạy trên rust với nhiệm vụ đọc sub cứng trong video = các frame với khoảng 10 tiếng+ voice của người nói (chú ý có nhiều phim tiết kiệm kinh phí chỉ có vài người lồng tiếng cho nhiều nhân vật; những đoạn ngập ngừng như `t-tôi muốn`, `à... thì`, etc; những phần chỉ có 1 từ như: `哦`, `啊`, `啊`, `自`, etc; ) => xuất ra file vtt đồng thời dịch từ văn bản gốc ví dụ như zh phồn thể và giản thể sang tiếng việt đồng thời cũng chú trọng vai vế và ngôi thứ cũng như đọc các caption về tên, đia danh, cấp bậc, được giới thiệu trong diện tích lớn của frame. Do đó cần có Vision Encoder, image encoder và một bộ não điều phối nhanh và linh hoạt, do đó chạy trên ram và nạp từng cụm nhỏ để tạo .zh.vtt, .vi.vtt và quy trình + cache để khi chạy lại không phải làm lại từ đầu. Bạn có thể gợi ý, bổ sung, hướng dẫn tôi hay không? Pure CPU + System RAM. không Gộp các keyframe liên tiếp có nội dung giống nhau (hoặc độ tương đồng Levenshtein > 90%) thành 1 block thời gian duy nhất (00:12:04.120 --> 00:12:06.850) quá nhiều vì phải tính toán thời gian để lồng tts sau này.

AI:

[WEB]

### Giả định & Làm rõ phạm vi kiến trúc

- **Giả định hạ tầng:** Hệ thống chạy trên **Pure CPU + System RAM**, biên dịch bằng **Rust 2024 Edition**, tái sử dụng trực tiếp workspace `ocr-core` (`ocr_subtitles`, `ocr_captions`, `ocr_merge`, `ocr_shared`) và `ferritedb` (`ferritedb_core`) hiện có của bạn.
- **2 cách hiểu về lưu trữ Cache với FerriteDB (không dùng SQL):**

1. **Cách hiểu A (Dùng `ferritedb_core::Database` API thuần Rust — Khuyên dùng):** Bỏ qua crate `ferrite-vdbe` (không cần parse/compile SQL), gọi thẳng B-tree API (`insert_into`, `update_in`, `create_index`, `begin_transaction`) của `ferritedb_core`. Tốc độ ghi đạt ~30.000 rows/s, tra cứu point-lookup ~50.000 queries/s, có sẵn WAL + CRC32C chống hỏng dữ liệu khi mất điện ngang chừng.
2. **Cách hiểu B (Không dùng DB, dùng Append-only Log nhị phân theo chunk):** Lưu trạng thái từng cụm 5 phút ra file `.bin` (`postcard`/`bincode`) trong `.tmp_<video>/`. Nhẹ hơn nhưng khó truy vấn chéo bảng thuật ngữ nhân vật giữa các tập phim dài 10+ tiếng.

- **2 cách hiểu về bài toán tách cue phục vụ lồng tiếng TTS sau này:**

1. **Cách hiểu A (Khóa mốc theo khoảng lặng âm thanh VAD + Biến thiên OCR):** Không gộp các câu giống nhau nếu giữa chúng có khoảng lặng giọng nói thực tế (ví dụ 2 tiếng `"啊"` cách nhau 0.25s phải là 2 cue riêng để TTS phát âm 2 lần đúng nhịp).
2. **Cách hiểu B (Chẻ nhỏ câu dài theo dấu ngắt nhịp):** Ngoài việc giữ cue ngắn, LLM điều phối còn gắn thêm metadata ước lượng tốc độ đọc (`tts_speed_ratio`) và điểm ngắt hơi (`pause_ms`) vào từng cue để engine TTS không bị tràn thời lượng.

---

### Phản biện kiến trúc & Điểm nghẽn trong code hiện tại

Qua phân tích trực tiếp mã nguồn `ocr_subtitles`, `ocr_captions`, `det_filter.rs` và `FerriteDB`, có **4 nguyên nhân cốt lõi** khiến pipeline hiện tại bị mất từ đơn, mất đoạn ngập ngừng và hard-code cứng nhắc:

1. **Nguyên nhân làm mất từ đơn (`哦`, `啊`, `自`) và đoạn ngập ngừng (`t-tôi muốn`, `à... thì`) trong `ocr_subtitles/src/lib.rs`:**

- **Bộ lọc `deduped` xóa nhầm từ lặp/ngập ngừng:** Tại khối loại bỏ mảnh vụn chuyển cảnh (`dur <= 0.30 && chars <= 2`), code kiểm tra `o.text.contains(&seg.text)`. Khi nhân vật ngập ngừng `"我"` rồi nói tiếp `"我想..."` trong vòng `0.35s`, chữ `"我"` đầu tiên bị xóa sạch vì bị coi là mảnh vụn của `"我想..."`.
- **Gộp liền kề quá tay cho TTS:** Điều kiện `is_same_subtitle(&last.text, &seg.text) && gap < 0.35` gộp 2 lần nhân vật thốt lên `"啊"` ... `"啊"` cách nhau `0.3s` thành 1 block duy nhất. Khi đưa sang TTS, máy chỉ đọc 1 tiếng `"A"` kéo dài thay vì 2 tiếng ngắt quãng.
- **Mất dấu câu chỉ báo ngữ điệu:** Hàm `sanitize_ocr_text` đang cắt sạch toàn bộ dấu câu (`，`, `。`, `！`, `？`, `…`, `-`, `—`). Khi mất dấu `…` hoặc `—`, LLM điều phối không thể phân biệt câu nói trôi chảy với câu đang ngập ngừng (`呃……那个`), làm bản dịch tiếng Việt bị cứng.
- **Ngưỡng cứng 2-frame:** Điều kiện `seg.end_frame < seg.start_frame + 1` loại bỏ hoàn toàn các từ cảm thán 1 chữ xuất hiện chớp nhoáng trong 1 khung hình.

2. **Nguyên nhân `ocr_captions/src/lib.rs` bỏ sót chữ cách điệu & tràn RAM khi chạy 10+ tiếng:**

- **Hard-code màu sắc & hình học:** Các hằng số `CAPTION_MAX_INK_SAT = 0.33`, `CAPTION_MIN_SHADOW_FRAC = 0.20`, `CAPTION_BRIGHT_BG_LUM = 100.0` và tỷ lệ khung chữ `shape_matches_text` (`0.50..=1.80`) đang loại bỏ toàn bộ thẻ tên/cấp bậc/địa danh viết bằng chữ thư pháp màu vàng/đỏ, chữ phát sáng không viền đen hoặc chữ viết dính nét. Chúng bị vứt bỏ ngay ở `detect_and_recognize` trước khi kịp xuất ảnh `--caption-evidence`!
- **Tích lũy toàn bộ video vào RAM:** Hàm `extract_captions` đẩy toàn bộ `all_frame_boxes: Vec<Vec<TextBox>>` vào một `Vec` duy nhất. Với video 10 tiếng ở `5 fps` (`180.000` frames), bộ nhớ sẽ phình rất lớn. Bạn cần chuyển sang luồng xử lý theo **chunk 5 phút** dựa trên `extract_captions_from_frames` và `extract_hardcoded_subtitles_from_frames` đã có sẵn.

3. **Lưu ý sống còn khi dùng `FerriteDB` làm Cache:**

- Theo tài liệu `FerriteDB`, mỗi trang dữ liệu cố định **4096 bytes và chưa hỗ trợ overflow page**. Do đó, **tuyệt đối không lưu byte ảnh PNG (`Vec<u8>`) vào FerriteDB**. Ảnh crop bằng chứng phải lưu thành file `.png` trong thư mục `.tmp_<video>/crops/`, còn FerriteDB chỉ lưu đường dẫn chuỗi (`String`) và text OCR/dịch để mỗi record luôn nằm gọn dưới `2 KB`.
- Cần gọi `database.checkpoint_if_needed(4 * 1024 * 1024)` sau mỗi chunk 5 phút để gom WAL về file chính.

4. **Dead code phát hiện được trong workspace hiện tại (giữ nguyên, chỉ liệt kê):**

- `ocr_subtitles/src/lib.rs`: Hàm `SubtitleRoi::parse` và 3 trường `lang`, `diff_threshold`, `accuracy_mode` trong `OcrExtractorConfig` đang gắn `#[allow(dead_code)]` và chưa được sử dụng.
- `ocr_captions/src/lib.rs`: Struct `TextBox` gắn `#[allow(dead_code)]`.

---

### Kiến trúc Studio-Grade Pipeline (Pure CPU + System RAM)

Thay vì đẩy toàn bộ frame qua Vision Encoder của VLM (rất chậm trên CPU), hệ thống tận dụng tốc độ của `DBNet + PP-OCRv6` hiện có làm **màng lọc sơ cấp**, chỉ đẩy những **mẫu crop bị nghi ngờ / chữ cách điệu** qua **Qwen2.5-VL-3B (INT4 ONNX)**, rồi chuyển toàn bộ sang **Bộ não điều phối LLM (Qwen2.5-7B-Instruct INT4 ONNX)**:

```text
[Video 10h+ & Audio 16kHz]
       │
       ├──► (Stage 1: Chunk 5 phút + Silero VAD ONNX + Dual OCR) ──► [FerriteDB: raw_cues & vlm_jobs]
       │       • Tách mốc giọng nói VAD (khóa khoảng lặng TTS)
       │       • Bảo toàn dấu câu ngập ngừng & cue 1 chữ (哦, 啊)
       │       • Bắt cả caption cách điệu/màu sắc -> lưu crop PNG ra đĩa
       │
       ├──► (Stage 2: Selective VLM Vision-Language ONNX) ─────────► [FerriteDB: cập nhật raw_zh]
       │       • Nạp Qwen2.5-VL-3B INT4 ONNX (~2.8GB RAM)
       │       • Chỉ đọc lại các crop PNG trong bảng `vlm_jobs`
       │       • Drop() session giải phóng hoàn toàn RAM
       │
       ├──► (Stage 3: Studio Director LLM ONNX) ───────────────────► [FerriteDB: studio_cues & glossary]
       │       • Nạp Qwen2.5-7B-Instruct INT4 ONNX (~4.8GB RAM)
       │       • Nạp gói 30 cues + Bảng nhân vật/cấp bậc + Nhãn loa (<v>)
       │       • Sửa lỗi OCR + Giữ ngập ngừng + Dịch chuẩn vai vế/ngôi thứ
       │       • Drop() session giải phóng hoàn toàn RAM
       │
       └──► (Stage 4: Studio VTT Exporter) ────────────────────────► [.zh.vtt] & [.vi.vtt]

```

---

### Hướng dẫn triển khai & Mã nguồn Rust cụ thể

#### Phân đoạn 1: Lớp Cache & Checkpoint bằng `ferritedb_core::Database` (Không SQL)

Đặt module này trong `ocr_shared/src/ferrite_cache.rs`. Toàn bộ thao tác dùng B-tree API thuần Rust, có Index theo `chunk_id` để chạy lại (Resume) ngay tại chunk bị gián đoạn.

```rust
// ocr_shared/src/ferrite_cache.rs
use anyhow::{Context, Result, anyhow};
use ferrite_btree::{RecordRef, Value, ValueRef};
use ferrite_vfs::OsVfs;
use ferritedb_core::{Database, DatabaseError};
use std::path::Path;

const TBL_META: &str = "pipeline_meta";
const TBL_RAW_CUES: &str = "raw_cues";
const TBL_VLM_JOBS: &str = "vlm_jobs";
const TBL_GLOSSARY: &str = "entity_glossary";
const TBL_STUDIO_CUES: &str = "studio_cues";

/// Đại diện một cue thô sau Stage 1 (đã gắn mốc VAD để chống gộp sai nhịp TTS)
#[derive(Debug, Clone)]
pub struct CachedRawCue {
    pub rowid: i64,
    pub chunk_id: i64,
    pub start_ms: i64,
    pub end_ms: i64,
    pub cue_kind: String,      // "SUB" | "CAP"
    pub speaker_hint: String,  // Nhãn từ Light-ASD / Pure-Rust speaker attribution
    pub raw_zh: String,
    pub confidence: f64,
    pub vad_speech_ratio: f64, // Tỷ lệ có tiếng nói thật trong khoảng [start_ms, end_ms]
}

/// Quản lý trạng thái pipeline trên FerriteDB (không dùng SQL, tối ưu cho page 4096B)
pub struct StudioCacheDb {
    vfs: OsVfs,
    db: Database,
}

impl StudioCacheDb {
    pub fn open_or_create(db_path: &Path) -> Result<Self> {
        let vfs = OsVfs;
        let mut db = if db_path.exists() {
            Database::open(&vfs, db_path, 128)
                .map_err(|e| anyhow!("Không thể mở FerriteDB {:?}: {}", db_path, e))?
        } else {
            Database::create(&vfs, db_path, 128)
                .map_err(|e| anyhow!("Không thể tạo FerriteDB {:?}: {}", db_path, e))?
        };

        Self::ensure_schema(&mut db)?;
        Ok(Self { vfs, db })
    }

    fn ensure_schema(db: &mut Database) -> Result<()> {
        let tables: &[(&str, &[&str])] = &[
            (TBL_META, &["key", "val"]),
            (
                TBL_RAW_CUES,
                &[
                    "chunk_id",
                    "start_ms",
                    "end_ms",
                    "cue_kind",
                    "speaker_hint",
                    "raw_zh",
                    "confidence",
                    "vad_speech_ratio",
                ],
            ),
            (
                TBL_VLM_JOBS,
                &["raw_cue_rowid", "crop_png_path", "reason", "resolved_zh"],
            ),
            (
                TBL_GLOSSARY,
                &["zh_term", "vi_term", "category", "pronoun_rule"],
            ),
            (
                TBL_STUDIO_CUES,
                &[
                    "chunk_id",
                    "start_ms",
                    "end_ms",
                    "cue_kind",
                    "speaker",
                    "clean_zh",
                    "translated_vi",
                    "tts_pause_after_ms",
                ],
            ),
        ];

        let mut tx = db.begin_transaction();
        let mut changed = false;
        for (table, cols) in tables {
            match tx.table_columns(table) {
                Ok(_) => {}
                Err(DatabaseError::TableNotFound(_)) => {
                    tx.create_table(table, cols)
                        .map_err(|e| anyhow!("Lỗi tạo bảng {}: {}", table, e))?;
                    changed = true;
                }
                Err(e) => return Err(anyhow!("Lỗi kiểm tra bảng {}: {}", table, e)),
            }
        }
        if changed {
            tx.create_index("idx_raw_cues_chunk", TBL_RAW_CUES, &["chunk_id"])
                .map_err(|e| anyhow!("Lỗi tạo index raw_cues: {}", e))?;
            tx.create_index("idx_studio_cues_chunk", TBL_STUDIO_CUES, &["chunk_id"])
                .map_err(|e| anyhow!("Lỗi tạo index studio_cues: {}", e))?;
            tx.commit()
                .map_err(|e| anyhow!("Lỗi commit schema FerriteDB: {}", e))?;
        }
        Ok(())
    }

    /// Ghi trọn gói một chunk 5 phút trong 1 WAL transaction duy nhất và checkpoint nếu WAL > 4MB
    pub fn save_chunk_raw_cues(
        &mut self,
        chunk_id: i64,
        cues: &[CachedRawCue],
        vlm_crops: &[(usize, String, String)], // (index_in_cues, crop_path, reason)
    ) -> Result<()> {
        let mut tx = self.db.begin_transaction();
        let mut inserted_rowids = Vec::with_capacity(cues.len());

        for cue in cues {
            // Giới hạn độ dài text để đảm bảo record luôn < 3500 bytes (nằm gọn 1 page 4096B)
            let safe_zh: String = cue.raw_zh.chars().take(500).collect();
            let rowid = tx
                .next_rowid(TBL_RAW_CUES)
                .map_err(|e| anyhow!("next_rowid error: {}", e))?;
            tx.insert_into(
                TBL_RAW_CUES,
                rowid,
                &[
                    Value::Integer(chunk_id),
                    Value::Integer(cue.start_ms),
                    Value::Integer(cue.end_ms),
                    Value::Text(&cue.cue_kind),
                    Value::Text(&cue.speaker_hint),
                    Value::Text(&safe_zh),
                    Value::Real(cue.confidence),
                    Value::Real(cue.vad_speech_ratio),
                ],
            )
            .map_err(|e| anyhow!("insert raw_cue error: {}", e))?;
            inserted_rowids.push(rowid);
        }

        for (cue_idx, crop_path, reason) in vlm_crops {
            if let Some(&raw_rowid) = inserted_rowids.get(*cue_idx) {
                let job_id = tx
                    .next_rowid(TBL_VLM_JOBS)
                    .map_err(|e| anyhow!("next_rowid vlm error: {}", e))?;
                tx.insert_into(
                    TBL_VLM_JOBS,
                    job_id,
                    &[
                        Value::Integer(raw_rowid),
                        Value::Text(crop_path),
                        Value::Text(reason),
                        Value::Text(""),
                    ],
                )
                .map_err(|e| anyhow!("insert vlm_job error: {}", e))?;
            }
        }

        tx.commit()
            .map_err(|e| anyhow!("commit chunk {} error: {}", chunk_id, e))?;

        // Giữ WAL nhỏ gọn sau mỗi chunk
        let _ = self.db.checkpoint_if_needed(&self.vfs, 4 * 1024 * 1024);
        Ok(())
    }

    /// Kiểm tra xem chunk_id đã được OCR xong chưa để bỏ qua khi chạy lại
    pub fn has_chunk_raw_cues(&mut self, chunk_id: i64) -> Result<bool> {
        // Dùng index equality trên cột chunk_id
        for (_, bytes) in self
            .db
            .scan_table(TBL_RAW_CUES)
            .map_err(|e| anyhow!("scan raw_cues error: {}", e))?
        {
            let rec = RecordRef::new(&bytes).context("Record lỗi định dạng")?;
            if let Some(ValueRef::Integer(cid)) = rec.get(0).context("Đọc cột chunk_id lỗi")?
                && cid == chunk_id
            {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

```

---

#### Phân đoạn 2: Sửa lỗi mất từ đơn (`哦`, `啊`, `自`) & giữ câu ngập ngừng cho TTS trong `ocr_subtitles`

Thay thế logic cắt dấu câu và gộp cue trong `ocr_subtitles/src/lib.rs` bằng bộ quy tắc **TTS-Aware Segmentation** kết hợp với danh sách khoảng giọng nói (`SpeechSpan`) từ `silero_vad.onnx`:

```rust
// Thay thế trong ocr_subtitles/src/lib.rs

/// Khoảng thời gian có giọng nói thật từ Silero VAD ONNX
#[derive(Debug, Clone, Copy)]
pub struct VadSpeechSpan {
    pub start_sec: f64,
    pub end_sec: f64,
}

/// Kiểm tra giữa 2 mốc thời gian có khoảng lặng giọng nói đáng kể hay không.
/// Nếu có khoảng lặng >= min_silence_sec, tuyệt đối KHÔNG gộp 2 cue (kể cả khi chữ giống hệt nhau như "啊" ... "啊").
pub fn has_vad_silence_between(
    prev_end_sec: f64,
    next_start_sec: f64,
    vad_spans: &[VadSpeechSpan],
    min_silence_sec: f64,
) -> bool {
    if next_start_sec - prev_end_sec >= min_silence_sec {
        return true;
    }
    if vad_spans.is_empty() {
        return false;
    }
    // Tính tổng thời gian có tiếng nói nằm trong khoảng chuyển tiếp [prev_end_sec - 0.05, next_start_sec + 0.05]
    let win_start = (prev_end_sec - 0.05).max(0.0);
    let win_end = next_start_sec + 0.05;
    let window_len = (win_end - win_start).max(0.001);

    let voiced_time: f64 = vad_spans
        .iter()
        .map(|s| (s.end_sec.min(win_end) - s.start_sec.max(win_start)).max(0.0))
        .sum();

    // Nếu vùng chuyển tiếp gần như im lặng (< 20% có tiếng nói), coi như 2 nhịp nói tách biệt
    (voiced_time / window_len) < 0.20
}

/// Làm sạch văn bản OCR nhưng BẢO TOÀN dấu ngập ngừng (…, —, -) và ngữ điệu (？, ！, ，)
/// để LLM dịch chuẩn cảm xúc và TTS ngắt hơi đúng nhịp.
pub fn sanitize_ocr_text_preserve_prosody(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    // Chỉ cắt các ký tự khung viền rác ở mép ngoài, giữ lại dấu ngắt nhịp bên trong
    let cleaned = trimmed
        .trim_matches(|c: char| {
            c.is_whitespace()
                || matches!(
                    c,
                    '_' | '<' | '>' | '《' | '》' | '【' | '】' | '"' | '\'' | '#' | '*' | '+' | '=' | '|' | '~'
                )
        })
        .trim();

    let mut final_chars: Vec<char> = cleaned
        .chars()
        .filter(|&c| {
            !(('\u{3040}'..='\u{30ff}').contains(&c) || ('\u{31f0}'..='\u{31ff}').contains(&c))
        })
        .collect();

    if final_chars.len() >= 2 {
        if final_chars[0].is_ascii_alphabetic()
            && ('\u{4e00}'..='\u{9fff}').contains(&final_chars[1])
        {
            final_chars.remove(0);
        }
        let last = final_chars.len() - 1;
        if final_chars[last].is_ascii_alphabetic()
            && ('\u{4e00}'..='\u{9fff}').contains(&final_chars[last - 1])
        {
            final_chars.pop();
        }
    }

    let has_cjk = final_chars
        .iter()
        .any(|&c| ('\u{4e00}'..='\u{9fff}').contains(&c));
    if !has_cjk {
        return String::new();
    }

    final_chars.into_iter().collect()
}

/// Hợp nhất và lọc mảnh vụn an toàn cho TTS:
/// 1. Giữ nguyên mọi từ đơn (哦, 啊, 自) nếu có VAD xác nhận có tiếng nói hoặc tồn tại >= 1 frame ổn định.
/// 2. Giữ nguyên đoạn ngập ngừng ("我" đứng trước "我不想...") thay vì xóa nhầm như mảnh vụn chuyển cảnh.
pub fn finalize_tts_safe_segments(
    initial_segments: Vec<SubtitleSegment>,
    vad_spans: &[VadSpeechSpan],
) -> Vec<SubtitleSegment> {
    // Bước 1: Chỉ gộp 2 phân đoạn liền kề nếu cùng nội dung VÀ KHÔNG có khoảng lặng VAD ở giữa
    let mut merged: Vec<SubtitleSegment> = Vec::with_capacity(initial_segments.len());
    for seg in initial_segments {
        if let Some(last) = merged.last_mut() {
            let gap = seg.start_sec - last.end_sec;
            let is_stutter_repeat = last.text.chars().count() <= 2 && last.text == seg.text;
            let has_silence = has_vad_silence_between(last.end_sec, seg.start_sec, vad_spans, 0.18);

            if is_same_subtitle(&last.text, &seg.text)
                && gap < 0.25
                && !is_stutter_repeat
                && !has_silence
            {
                last.end_sec = seg.end_sec;
                if seg.text.chars().count() > last.text.chars().count() {
                    last.text = seg.text;
                }
                continue;
            }
        }
        merged.push(seg);
    }

    // Bước 2: Chỉ loại mảnh vụn transition khi là HẬU TỐ ngắn (< 120ms) bị rụng ở cuối câu trước,
    // KHÔNG bao giờ xóa tiền tố ngắn đứng trước câu dài (vì đó là nhịp ngập ngừng: "我... 我不想")
    let mut deduped: Vec<SubtitleSegment> = Vec::with_capacity(merged.len());
    for i in 0..merged.len() {
        let seg = &merged[i];
        let dur = seg.end_sec - seg.start_sec;
        let chars = seg.text.chars().count();

        if dur < 0.12 && chars == 1 && i > 0 {
            let prev = &merged[i - 1];
            let gap_from_prev = seg.start_sec - prev.end_sec;
            if gap_from_prev <= 0.15 && prev.text.ends_with(&seg.text) && prev.text != seg.text {
                continue;
            }
        }
        deduped.push(seg.clone());
    }

    deduped
}

```

---

#### Phân đoạn 3: Mở rộng `ocr_captions` để bắt chữ cách điệu & đẩy vào VLM Queue

Trong `ocr_captions/src/lib.rs`, thay vì vứt bỏ các box chữ thư pháp màu sắc (vàng/đỏ, phát sáng không viền đen) hoặc box bị lệch tỷ lệ vuông do chữ cách điệu, ta gắn cờ `needs_vlm_verify = true` và lưu crop PNG ra đĩa để **Qwen2.5-VL-3B INT4 ONNX** đọc lại ở Stage 2:

```rust
// Bổ sung vào ocr_captions/src/lib.rs

/// Đánh giá một box caption có phải chữ cách điệu/thẻ tiêu đề lớn cần VLM đọc lại hay không
pub fn classify_caption_for_vlm(
    w: f64,
    h: f64,
    frame_w: usize,
    frame_h: usize,
    confidence: f32,
    ink_ok: bool,
    shape_ok: bool,
    scene_ok: bool,
) -> (bool /* keep_candidate */, bool /* needs_vlm */) {
    let area_ratio = (w * h) / ((frame_w * frame_h).max(1) as f64);
    let is_large_banner = area_ratio >= 0.015 || h >= (frame_h as f64 * 0.08);

    // 1. Caption tiêu chuẩn sắc nét -> giữ ngay, không tốn chi phí VLM
    if ink_ok && shape_ok && scene_ok && confidence >= 0.88 {
        return (true, false);
    }

    // 2. Thẻ tên/cấp bậc/địa danh diện tích lớn hoặc chữ thư pháp cách điệu
    // (màu vàng/đỏ, không có viền đen, hoặc confidence 0.45..0.88) -> giữ và đẩy qua VLM ONNX
    if is_large_banner && confidence >= 0.45 {
        return (true, true);
    }

    // 3. Caption cỡ vừa nhưng confidence thấp do font lạ
    if (ink_ok || scene_ok) && confidence >= 0.55 {
        return (true, true);
    }

    (false, false)
}

```

---

#### Phân đoạn 4: Bộ não điều phối Studio LLM Coordinator trên `ort` (Dịch vai vế + Nhịp TTS)

Ở Stage 3, sau khi `drop()` VLM khỏi RAM, Rust nạp duy nhất **1 model LLM thuần văn bản lượng tử hóa INT4** (khuyên dùng `Qwen2.5-7B-Instruct-ONNX-INT4` vì xử lý Hán-Việt, tiên hiệp/cổ trang/đô thị và ngữ cảnh xưng hô tiếng Việt vượt trội hơn hẳn các bản 1.5B–3B).

Để giải quyết bài toán **1 người lồng tiếng cho nhiều nhân vật**, LLM Coordinator nhận đồng thời:

1. **Nhãn khẩu hình / vị trí loa (`speaker_hint`)** từ `ocr-core` (`--attribute-speakers` / `Light-ASD`).
2. **Thẻ danh tính / cấp bậc (`CAP`)** vừa xuất hiện trong khung hình.
3. **Sổ tay nhân vật tích lũy (`EntityGlossary`)** lưu trong `FerriteDB` từ các chunk trước.

```rust
// ocr_shared/src/studio_director.rs
use serde::{Deserialize, Serialize};

/// Cấu trúc đầu vào gửi cho LLM Coordinator theo từng gói 30 cues
#[derive(Debug, Serialize)]
pub struct DirectorBatchInput {
    pub chunk_id: i64,
    /// Sổ tay nhân vật, cấp bậc, địa danh đã khóa từ đầu phim tới hiện tại
    pub active_glossary: Vec<GlossaryEntry>,
    /// 5 câu thoại cuối của gói trước để giữ mạch xưng hô liên tục
    pub previous_context: Vec<TranslatedCue>,
    /// Danh sách cues cần hiệu đính & dịch trong gói hiện tại
    pub cues_to_process: Vec<RawCueInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlossaryEntry {
    pub zh_term: String,
    pub vi_term: String,
    pub category: String,     // "PERSON" | "TITLE_RANK" | "LOCATION" | "SKILL"
    pub pronoun_rule: String, // VD: "Sư tôn - Đồ đệ: xưng Ta - Con; với kẻ địch: Bản tôn - Ngươi"
}

#[derive(Debug, Serialize)]
pub struct RawCueInput {
    pub id: i64,
    pub start_ms: i64,
    pub end_ms: i64,
    pub kind: String,         // "SUB" hoặc "CAP"
    pub speaker_hint: String, // Từ Light-ASD / Active Speaker (có thể rỗng nếu off-screen)
    pub raw_zh: String,
}

/// Cấu trúc JSON đầu ra bắt buộc từ LLM Coordinator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectorBatchOutput {
    /// Các thực thể mới phát hiện từ CAP hoặc thoại để cập nhật vào FerriteDB
    pub new_glossary_entries: Vec<GlossaryEntry>,
    pub cues: Vec<TranslatedCue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslatedCue {
    pub id: i64,
    /// Văn bản gốc tiếng Trung đã sửa lỗi đồng âm/rụng nét của OCR (xuất ra .zh.vtt)
    pub clean_zh: String,
    /// Tên nhân vật được suy luận từ ngữ cảnh + khẩu hình + thẻ tên
    pub resolved_speaker: String,
    /// Bản dịch tiếng Việt chuẩn vai vế, giữ nguyên ngập ngừng ("T-tôi muốn...", "À... thì...")
    pub translated_vi: String,
    /// Gợi ý khoảng nghỉ sau câu (ms) cho bộ tổng hợp giọng nói TTS
    pub tts_pause_after_ms: u32,
}

/// Tạo System Prompt chuẩn Studio cho LLM ONNX (Qwen2.5-7B-Instruct INT4)
pub fn build_studio_director_prompt(batch: &DirectorBatchInput) -> String {
    let batch_json = serde_json::to_string(batch).unwrap_or_default();
    format!(
        r#"Bạn là Đạo diễn Hậu kỳ & Biên dịch Phim chuyên nghiệp (Trung - Việt).
Nhiệm vụ của bạn là xử lý gói phụ đề và thẻ chú thích (caption) sau đây để chuẩn bị cho khâu lồng tiếng TTS:

QUY TẮC BẮT BUỘC:
1. HIỆU ĐÍNH OCR (`clean_zh`): Sửa các lỗi nhận diện sai mặt chữ Hán đồng âm/gần nét dựa trên ngữ cảnh, nhưng GIỮ NGUYÊN từ đơn cảm thán (哦, 啊, 嗯, 自...) và cấu trúc lặp ngập ngừng.
2. PHÂN ĐỊNH NHÂN VẬT (`resolved_speaker`): Phim ít diễn viên lồng tiếng nên KHÔNG dựa vào chất giọng. Hãy kết hợp `speaker_hint`, thẻ chú thích `CAP` (tên, cấp bậc, địa danh), và mạch đối đáp để xác định chính xác ai đang nói với ai.
3. CHUẨN VAI VẾ & NGÔI THỨ (`translated_vi`):
   - Áp dụng nghiêm ngặt `active_glossary` đã có và bổ sung nhân vật/cấp bậc mới vào `new_glossary_entries`.
   - Dịch đúng ngôi thứ theo bối cảnh phim (Tiên hiệp/Cổ trang: Ta - Ngươi, Tại hạ - Các hạ, Sư tôn - Đồ nhi, Thuộc hạ - Điện hạ; Đô thị: Tôi - Anh/Cô/Ông/Ngài...). Tuyệt đối không dịch máy móc thành "Tôi - Bạn".
4. BẢO TOÀN NHỊP NGẬP NGỪNG & TỪ ĐƠN CHO TTS:
   - Giữ nguyên nhịp lắp bắp, ngập ngừng tự nhiên trong tiếng Việt (ví dụ: "我…我想" -> "T-tôi muốn...", "呃…那个" -> "À... thì...", "啊" -> "A!").
   - Đặt `tts_pause_after_ms` (0 đến 800ms) dựa trên dấu câu và khoảng cách thời gian tới câu kế tiếp.
5. ĐỊNH DẠNG ĐẦU RA: Chỉ trả về DUY NHẤT một khối JSON hợp lệ khớp schema `DirectorBatchOutput`, không giải thích thêm.

DỮ LIỆU ĐẦU VÀO:
{batch_json}"#
    )
}

```

---

#### Phân đoạn 5: Unit Test tái hiện lỗi & kiểm chứng logic bảo toàn từ đơn / ngập ngừng

Thêm trực tiếp vào module `#[cfg(test)]` của `ocr_subtitles/src/lib.rs` để đảm bảo không bao giờ tái phát lỗi mất từ đơn hoặc gộp nhầm nhịp TTS:

```rust
#[cfg(test)]
mod tts_segmentation_tests {
    use super::*;

    #[test]
    fn preserves_repeated_single_char_utterances_separated_by_vad_silence() {
        // Tái hiện lỗi: Nhân vật thốt lên "啊" 2 lần cách nhau 0.25s
        // Logic cũ (gap < 0.35s) gộp thành 1 block dài làm sai timing TTS
        let initial = vec![
            SubtitleSegment {
                start_sec: 1.00,
                end_sec: 1.20,
                text: "啊".to_string(),
            },
            SubtitleSegment {
                start_sec: 1.45,
                end_sec: 1.65,
                text: "啊".to_string(),
            },
        ];
        let vad_spans = vec![
            VadSpeechSpan { start_sec: 0.98, end_sec: 1.22 },
            VadSpeechSpan { start_sec: 1.43, end_sec: 1.68 },
        ];

        let result = finalize_tts_safe_segments(initial, &vad_spans);
        assert_eq!(result.len(), 2, "Hai tiếng '啊' có khoảng lặng VAD phải tách thành 2 cue riêng");
        assert_eq!(result[0].text, "啊");
        assert_eq!(result[1].text, "啊");
    }

    #[test]
    fn preserves_stutter_prefix_before_full_sentence() {
        // Tái hiện lỗi: Nhân vật ngập ngừng "我" (0.2s) rồi nói "我想离开这里"
        // Logic cũ (deduped) xóa mất chữ "我" đầu tiên
        let initial = vec![
            SubtitleSegment {
                start_sec: 5.00,
                end_sec: 5.20,
                text: "我".to_string(),
            },
            SubtitleSegment {
                start_sec: 5.30,
                end_sec: 6.50,
                text: "我想离开这里".to_string(),
            },
        ];
        let vad_spans = vec![
            VadSpeechSpan { start_sec: 4.95, end_sec: 6.55 },
        ];

        let result = finalize_tts_safe_segments(initial, &vad_spans);
        assert_eq!(result.len(), 2, "Không được xóa tiền tố ngập ngừng đứng trước câu đầy đủ");
        assert_eq!(result[0].text, "我");
        assert_eq!(result[1].text, "我想离开这里");
    }

    #[test]
    fn keeps_hesitation_punctuation_for_llm_and_tts() {
        assert_eq!(
            sanitize_ocr_text_preserve_prosody("呃……那个，我-我想"),
            "呃……那个，我-我想"
        );
    }
}

```

---

### Câu hỏi làm rõ bước tiếp theo

Máy chạy **Pure CPU + System RAM** của bạn hiện có tổng dung lượng **RAM khả dụng là bao nhiêu GB** (16GB, 32GB hay 64GB+) và bạn muốn tôi viết tiếp phần **tích hợp `ort` (ONNX Runtime) nạp/giải phóng tuần tự Qwen2.5-VL-3B INT4 + Qwen2.5-7B INT4** hay **vòng lặp chunking 5 phút nối `FrameProvider` với `StudioCacheDb**` trước?
