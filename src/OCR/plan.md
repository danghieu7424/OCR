subtitleextractor.com (và các công cụ dịch thuật/trích xuất phụ đề hiện đại) đạt tỷ lệ nhận diện cao là nhờ kết hợp **Mô hình học sâu nhận dạng văn bản (Deep Learning OCR)** tiên tiến cùng với các bước **tiền xử lý hình ảnh chuyên sâu**, thay vì chỉ cắt khung hình (frame cropping) thô rồi đưa thẳng vào OCR truyền thống (như Tesseract mặc định).

Khi bạn chỉ cắt frame và dùng OCR thông thường, độ chính xác bị giảm mạnh (chỉ khoảng 60%) thường do **viền chữ bị nhòe (aliasing/blur), nền video phức tạp, độ tương phản thấp hoặc font chữ có kiểu dáng nghệ thuật/viền bóng (stroke/shadow)** khiến bộ OCR quét nhầm hoặc bỏ sót.

---

### Các kỹ thuật cốt lõi giúp Subtitle Extractor đạt độ chính xác cao

1. **Sử dụng Engine OCR chuyên dụng cho phụ đề (như PaddleOCR, TrOCR hoặc mô hình Vision-Language tùy chỉnh)**
* **PaddleOCR / PP-OCRv4:** Đây là thư viện mã nguồn mở rất phổ biến cho phụ đề video nhờ mô hình nhận diện văn bản hướng ngang/ngược cực tốt và có các trọng số (weights) được huấn luyện riêng trên tập dữ liệu phụ đề đa ngôn ngữ.
* **Transformer-based OCR (TrOCR):** Kết hợp giữa Encoder thị giác và Decoder ngôn ngữ, giúp mô hình "đoán" được từ tiếp theo dựa vào ngữ cảnh ngay cả khi hình ảnh ký tự bị mờ.


2. **Tiền xử lý ảnh chuyên sâu (Image Preprocessing Pipeline)**
Trước khi đưa vào OCR, mỗi khung hình đều trải qua chuỗi bước xử lý hình ảnh bằng OpenCV:
* **Khử nhiễu & Phân đoạn màu (Color Thresholding / Binarization):** Phụ đề phim thường là chữ trắng hoặc vàng có viền đen hoặc bóng mờ. Các công cụ này dùng thuật toán tách biệt hoàn toàn màu chữ ra khỏi nền video động phía sau (chuyển về ảnh đen trắng tuyệt đối - Binary Image).
* **Phóng đại kích thước (Upscaling / Super-Resolution):** Subtitle trong video gốc thường có độ phân giải thấp (ví dụ cắt ra chỉ cao 20-30 pixel). Việc dùng các thuật toán nội suy hoặc AI phóng to ảnh (Lanczos hoặc Real-ESRGAN nhẹ) giúp nét chữ sắc sảo hơn trước khi quét.
* **Khử viền bóng và hiệu ứng (Morphological Operations):** Dùng phép co/giãn ảnh (Dilate/Erode) để loại bỏ viền đen bao quanh chữ hoặc làm liền mạch các nét chữ bị đứt đoạn do nén video (artifact nén JPEG/MP4).


3. **Thuật toán gom cụm dòng (Text Detection & Line Grouping)**
* Thay vì nhận diện từng ký tự rời rạc, họ sử dụng mô hình phát hiện văn bản (như DBNet - Differentiable Binarization) để khoanh vùng chính xác tọa độ câu phụ đề, sau đó mới cắt vùng văn bản đó để đưa vào bước nhận diện từ (Text Recognition).



---

### Hướng dẫn tối ưu hóa mã nguồn OCR của bạn [HW-ELEC / NATIVE / WEB]

Nếu bạn đang tự viết code xử lý (ví dụ bằng Python + OpenCV + Tesseract/PaddleOCR), hãy áp dụng đoạn pipeline tiền xử lý sau đây trước khi gọi OCR để cải thiện tỷ lệ nhận diện từ 60% lên trên 90%:

```python
import cv2
import numpy as np

def preprocess_subtitle_frame(frame_path):
    # 1. Đọc ảnh xám (Grayscale)
    img = cv2.imread(frame_path)
    gray = cv2.cvtColor(img, cv2.COLOR_BGR2GRAY)
    
    # 2. Phóng to ảnh gấp 2 lần bằng phương pháp nội suy chất lượng cao
    resized = cv2.resize(gray, (0, 0), fx=2.0, fy=2.0, interpolation=cv2.INTER_CUBIC)
    
    # 3. Tăng độ tương phản mạnh bằng CLAHE (Contrast Limited Adaptive Histogram Equalization)
    clahe = cv2.createCLAHE(clipLimit=3.0, tileGridSize=(8, 8))
    contrast = clahe.apply(resized)
    
    # 4. Ngưỡng hóa thích ứng (Adaptive Thresholding) để loại bỏ nền video tạp chất
    # Giúp giữ lại chữ trắng/vàng nổi bật trên nền tối
    thresh = cv2.adaptiveThreshold(
        contrast, 255, 
        cv2.ADAPTIVE_THRESH_GAUSSIAN_C, 
        cv2.THRESH_BINARY, 11, 2
    )
    
    return thresh

# Sau đó truyền ảnh 'thresh' này vào engine OCR (Ví dụ: PaddleOCR hoặc Tesseract với --psm 7 hoặc 8 cho dòng đơn)

```

---