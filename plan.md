# Lộ trình OCR thuần

## Nguyên tắc

- Pipeline mặc định chỉ trích xuất subtitle/caption và merge bằng `ocr_merge`.
- VTT OCR thô là dữ liệu chuẩn của pipeline; không tự sửa bằng LLM, glossary hay fuzzy matching.
- Không thay đổi thuật toán lõi của `ocr_subtitles` và `ocr_captions` trong các hạng mục dưới đây.
- Đánh giá theo video và VTT tham chiếu đã kiểm chứng; không suy rộng một điểm số thành bảo đảm cho mọi đầu vào.

## Hiện trạng

- `ocr_subtitles` dùng PP-OCRv6, mặc định quét vùng gần đáy khung hình; profile ROI cần đổi nếu subtitle nằm ở vị trí khác.
- `ocr_captions` mặc định quét Y 10–80% và X 0–100%; vùng rộng có thể chứa chữ trong cảnh và watermark.
- `extract_and_merge` chạy hai OCR song song, giữ VTT thô riêng và merge trực tiếp.
- `--caption-evidence` xuất crop first/best/last có timestamp, manifest và danh sách cảnh báo theo confidence/thời lượng.
- Comparator hỗ trợ `.subtitles.zh.vtt` và `.zh.vtt`, báo riêng Conditional Character Accuracy và Overall CER.

## Các bước tiếp theo

### 1. Kiểm chứng VTT và cảnh báo

- Dùng `input/` làm ground truth chỉ khi VTT đã được người dùng kiểm tra.
- Với từng bộ video, lưu lệnh chạy, ROI, model/dictionary, phiên bản và thống kê.
- Duyệt ảnh evidence của cue caption bị cảnh báo và các sai lệch timestamp; không thay dữ liệu thô trong bước này.
- Giữ riêng biến thể chính tả/phồn-giản thể với lỗi OCR; chuẩn hóa chỉ được dùng như một góc nhìn đánh giá tùy chọn.

### 2. Cải thiện căn chỉnh đa khung hình có trọng số confidence

- Nghiên cứu căn chỉnh chuỗi giữa các frame theo thời gian, dùng confidence OCR làm trọng số.
- So sánh với kết quả hiện tại trên tập kiểm chứng cố định; đo CER, cue thiếu/thừa và sai số timestamp.
- Chỉ triển khai nếu cải thiện được tập giữ lại mà không làm giảm khả năng giữ cue ngắn.
- Không dùng majority vote theo chỉ số ký tự khi các frame có độ dài/căn chỉnh khác nhau.

### 3. Mở rộng profile đầu vào có kiểm soát

- Thu thập ví dụ cho các vị trí subtitle/caption, độ phân giải và font khác nhau.
- Ghi rõ model, ngôn ngữ, ROI và ngưỡng đã được kiểm chứng cho từng profile.
- Nếu video nằm ngoài profile hỗ trợ, yêu cầu chọn ROI/cấu hình hoặc duyệt thủ công thay vì hứa hẹn độ chính xác.

## Chỉ số chấp nhận

- **Conditional Character Accuracy**: chỉ trên các cặp cue đã căn; luôn kèm số ký tự và số cặp.
- **Overall CER**: gồm lỗi trên cue đã căn và toàn bộ ký tự cue thiếu/thừa; mẫu số là tổng ký tự tham chiếu.
- **Cue coverage**: số cue tham chiếu, OCR, cặp đã căn, thiếu và thừa.
- **Caption review**: độ khớp chữ, ảnh evidence đã duyệt và sai số đầu/cuối cue.

Mọi điểm số cần ghi rõ tập video, chuẩn hóa bật/tắt và cách căn cue. Không dùng kết quả của một series làm cam kết cho video bất kỳ.
