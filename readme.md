

---

### 🔤 1. Từ điển ký tự (`keys.txt`)

Model server có output shape là `[DynamicDimension.0, Reshape_485_00__d2, 18385]`. Con số **`18385`** chính là tổng số lớp ký tự (bao gồm cả ký tự trống - blank). Do đó, file từ điển của bạn phải có số dòng tương ứng (thường là 18384 dòng, vì lớp thứ 0 là blank).

File từ điển chuẩn cho PP-OCRv5 là **`ppocrv5_dict.txt`**. Bạn có thể tải từ bản phát hành chính thức của dự án OAR OCR:
- **Link tải:** [https://github.com/GreatV/oar-ocr/releases/download/v0.1.0/ppocrv5_dict.txt](https://github.com/GreatV/oar-ocr/releases/download/v0.1.0/ppocrv5_dict.txt)

Sau khi tải về, hãy đổi tên file thành `keys.txt` và ghi đè vào thư mục `models/ocr/` (nhớ xóa file `keys.txt` cũ của bản v4).

---

### 📥 2. Tải model `rec.onnx` bản server (nếu cần)

Nếu bạn chưa có file `rec.onnx` bản server, có thể tải từ kho lưu trữ của OAR OCR:
- **Link tải:** [https://github.com/GreatV/oar-ocr/releases/download/v0.1.0/ppocrv5_server_rec.onnx](https://github.com/GreatV/oar-ocr/releases/download/v0.1.0/ppocrv5_server_rec.onnx)

Sau khi tải, đổi tên thành `rec.onnx` và đặt vào `models/ocr/`.

---
