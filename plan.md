Để đạt mức **chuẩn tuyệt đối từng từ (100.00%)** phục vụ dịch thuật mà không bị dịch sai ngữ cảnh, bản thân mô hình OCR độc lập (dù là PaddleOCR v5 hay v6) **vẫn có giới hạn vật lý** khi gặp:

- Nền chuyển động phức tạp (hiệu ứng chưởng pháp, bùa chú, cháy nổ che khuất nét chữ).
- Các chữ Hán có cấu tạo cực kỳ giống nhau (như `爹` vs `参`, `嗯` vs `涵`, `己` vs `已`).

Để giải quyết triệt để vấn đề này trong công nghiệp sản xuất phụ đề, có **3 giải pháp chuẩn** bạn có thể áp dụng:

---

### 1. Dùng LLM Hậu xử lý (Post-processing Context Correction)

Đây là cách các hệ thống dịch thuật và làm sub hiện đại đang dùng hiệu quả nhất:

- **Cách làm**: Đưa toàn bộ dòng phụ đề thô vừa OCR xong qua một LLM nhẹ (chuyên tiếng Trung) với vai trò **"Hiệu đính ngữ cảnh" (Proofreading)** trước khi nạp vào máy dịch.
- **Hiệu quả**:
  - LLM hiểu ngữ pháp và câu chuyện: Thấy câu _"娘亲参參"_ hoặc _"参以前的武器给你"_, nó sẽ tự biết chữ `参` ở đây phải là `爹` (_"娘亲爹爹"_, _"爹以前的武器给你"_).
  - Thấy _"自已"_ nó sẽ tự sửa thành _"自己"_, thấy chữ rơi rụng hay nhầm âm sẽ được nắn lại chuẩn ngữ cảnh 100% trước khi đem đi dịch sang tiếng Việt.

---

### 2. Tận dụng từ điển danh từ riêng & Nhân vật (Glossary / Entity Dictionary)

Trong phim hoạt hình / tu tiên:

- Tên nhân vật, công pháp, địa danh là thứ **tuyệt đối không được sai** vì sai 1 chữ máy dịch sẽ dịch ra nghĩa kỳ quặc (ví dụ: `何訇`, `陆清安`, `九狱弑仙剑`, `无垢帝位`).
- **Cách làm**: Lập một bảng từ điển danh từ riêng của bộ phim (Glossary). Sau khi OCR xong, chạy thuật toán so khớp gần đúng (Fuzzy match) với danh sách nhân vật/địa danh:
  - Nếu OCR ra `陆安` hay `防安` trong khoảng thời gian có mặt nhân vật $\rightarrow$ tự động map về chuẩn `陆清安`.
  - Nếu OCR ra `帝境中期` ở tập 8 $\rightarrow$ map về `大帝境中期`.
  - Bộ từ điển này cũng dùng để khóa từ khi nạp vào model dịch, đảm bảo tên nhân vật đồng nhất từ tập 1 đến tập 100.

---

### 3. Cơ chế "Bỏ phiếu đa khung hình" (Multi-frame Temporal Voting)

- **Cách làm**: Thay vì chỉ lấy kết quả của 1 frame đại diện trong cụm, ta gom kết quả OCR của tất cả các frame trong khoảng thời gian dòng sub đó xuất hiện (ví dụ xuất hiện 1.5 giây ở 10fps là có 15 frame).
- **Hiệu quả**:
  - Ở frame bị hiệu ứng bùa chú che, chữ có thể bị đọc nhòe thành `挡一` hay `星 挡住`.
  - Nhưng ở 10 frame còn lại khi hiệu ứng tan bớt, nó đọc đúng là `挡住`.
  - Bằng cách phân tích tần suất từng ký tự trên toàn bộ 15 frame đó, hệ thống sẽ tự động ghép lại thành câu hoàn chỉnh và loại bỏ hoàn toàn các ký tự bị che khuất tạm thời.

---

### Tóm lại quy trình chuẩn để dịch hoàn hảo:

1. **Tầng 1 (OCR hiện tại)**: Đã làm rất tốt việc bóc tách và định vị mốc thời gian chuẩn xác đến từng mili-giây (~99.8%).
2. **Tầng 2 (Glossary + LLM Proofreader)**: Tự động nắn lại các chữ nhầm ngữ cảnh và chuẩn hóa danh từ riêng.
3. **Tầng 3 (Dịch thuật)**: Đưa bản phụ đề đã sạch 100% vào pipeline dịch tiếng Việt.

---

---

Dưới đây là câu trả lời chi tiết và thực tế cho 3 thắc mắc của bạn (không can thiệp code):

---

### 1. Về LLM Hậu xử lý (Chạy thuần CPU, cực nhẹ, đọc tốt tiếng Trung và nhận được ảnh - VLM)

Nếu bạn chạy **thuần CPU**, các mô hình Vision-Language (VLM) lớn sẽ rất chậm (vài giây đến hàng chục giây 1 ảnh). Để vừa nhẹ, vừa thông minh, lại tối ưu CPU, bạn nên chọn các dòng sau:

1. **Qwen2.5-VL-3B-Instruct (hoặc bản siêu nhẹ 1.5B sắp/mới ra mắt - định dạng GGUF / ONNX / OpenVINO)**:
   - **Lý do**: Đây là "ông vua" tiếng Trung hiện nay từ Alibaba. Khả năng nhận diện chữ Hán cách điệu, chữ viết tay, thư pháp và hiểu ngữ cảnh văn hóa/tu tiên của dòng Qwen là số 1 thế giới ở phân khúc nhỏ.
   - **Tốc độ CPU**: Bản `Qwen2-VL-2B` hoặc `Qwen2.5-VL-3B` lượng tử hóa 4-bit (`q4_k_m`) chạy qua `llama.cpp` chỉ tốn khoảng 2–3 GB RAM, CPU 8–12 core xử lý rất mượt.
2. **Moondream2 (1.86B parameters)**:
   - Cực kỳ nhẹ, thiết kế chuyên biệt để chạy CPU và biên nhúng (Edge). Nhận diện ảnh nhanh, tuy nhiên tiếng Trung cổ/tu tiên sẽ không sâu bằng dòng Qwen.
3. **Chiến lược tối ưu tốc độ CPU**:
   - **Không nạp toàn bộ ảnh phim**: Không đưa cả ảnh 1080p vào VLM.
   - **Chỉ nạp box crop**: Chỉ crop đúng vùng chữ bị nghi ngờ hoặc đưa văn bản thô + ảnh crop của duy nhất chữ đó vào cho mô hình "nhìn lại" để xác nhận, tốc độ trên CPU sẽ tăng gấp 5–10 lần.

---

### 2. Tự động hóa tạo Danh từ riêng / Nhân vật (Glossary hoàn toàn tự động)

Bạn **không cần gõ tay** danh sách này. Trong phim hoạt hình Trung Quốc (donghua), cấu trúc phim có một đặc tính tuyệt vời để tự động hóa:

1. **Khai thác chính module Caption bạn vừa hoàn thiện**:
   - Mọi nhân vật chính, địa danh lớn khi xuất hiện lần đầu đều có **thanh đen chú thích tên** (Name plate).
   - Ví dụ: `《陆清安》`, `《林清雪》`, `《徐清莹》`, `《大女儿-陆紫萱》`, `《九狱弑仙剑》`...
   - Hệ thống chỉ cần tự động gom tất cả các dòng nằm trong cặp ngoặc `《...》` của 20 tập lại $\rightarrow$ Bạn đã có ngay **90% bảng Danh từ riêng chuẩn xác nhất** của toàn bộ bộ phim mà không tốn 1 giây gõ tay.
2. **Dùng NER tự động (Named Entity Recognition - Nhận diện thực thể tên người/địa danh)**:
   - Chạy một mô hình NER tiếng Trung siêu nhẹ (như `jieba` gắn nhãn từ loại `nr` - tên người, `ns` - địa danh, hoặc mô hình bert-chinese nhỏ ~50MB).
   - Quét qua toàn bộ phụ đề đã OCR, lọc ra các cụm từ xuất hiện lặp lại nhiều lần mang danh xưng (như `...帝`, `...城`, `...剑`) để đưa vào bảng từ điển ưu tiên.

---

### 3. Về cơ chế "Bỏ phiếu đa khung hình" (Multi-frame Temporal Voting) trong code của bạn

Đúng như bạn nhớ, **trong code của bạn đã có nền tảng vững chắc của cơ chế này**, thể hiện rõ qua lịch sử `CHANGELOG.md` và kiến trúc hiện tại:

1. **Trong `src/caption.rs`**:
   - Đã áp dụng cơ chế gom cụm thời gian (`RawSeg`), sau đó dùng hàm `select_best_caption(&seg.texts)` để **đếm tần suất và chọn ra chuỗi văn bản chiếm đa số phiếu** trên toàn bộ các frame mà caption đó tồn tại.
   - Đây chính là lý do vì sao chữ `陆清安` dù có frame bị nhiễu thành `陆安` hay `防安`, kết quả cuối cùng vẫn ra chuẩn xác `陆清安` vì dạng đúng chiếm đa số phiếu.
2. **Trong `src/OCR/mod.rs` (Subtitle)**:
   - Đã có phân đoạn thời gian (`cluster_records`), thuật toán làm mịn (`temporal_smooth`), lấp chỗ trống 1–2 frame (`dropout fill`), và so khớp tương đồng chuỗi (`is_same_subtitle` bằng khoảng cách Levenshtein).
   - Điểm khác biệt duy nhất: Code phụ đề hiện tại đang chọn chuỗi có độ dài tốt nhất/ổn định nhất của cụm. Trong nhật ký `evaluate.md`, agent trước cũng đã ghi chú: _Nếu sau này muốn cứu những ca chữ bị che khuất tạm thời trên nền bùa chú như `挡住`, hướng đi chuẩn là nâng cấp bầu phiếu ký tự con theo cửa sổ lân cận $\pm 0.5\text{s}$ như cách Caption đang làm._
