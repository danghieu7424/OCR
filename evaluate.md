# Kết quả kiểm chứng OCR

Ngày chạy: 2026-11-15. Bảng dưới đây ghi lại lần chạy lại 20 tập S01 sau các thay đổi OCR, so sánh bằng `tools/compare_subs.py` trên VTT subtitle OCR thô, không chuẩn hóa phồn/giản thể (`--norm none`). Ground truth trong `input/` được giữ nguyên; đầu vào và kết quả chạy được lưu tạm dưới `scratch/s01-recheck-input/` và `scratch/s01-recheck-raw/`.

## Subtitle: 20 tập

| Tập | Ref cues | OCR cues | Cặp căn | Cue đúng | Thiếu | Thừa | Conditional Character Accuracy | Overall CER |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| 001 | 61 | 62 | 61 | 57 | 0 | 1 | 99.07% | 1.17% |
| 002 | 108 | 108 | 108 | 98 | 0 | 0 | 98.64% | 1.36% |
| 003 | 44 | 44 | 44 | 42 | 0 | 0 | 99.39% | 0.61% |
| 004 | 79 | 79 | 79 | 77 | 0 | 0 | 99.65% | 0.35% |
| 005 | 26 | 26 | 26 | 26 | 0 | 0 | 100.00% | 0.00% |
| 006 | 23 | 21 | 21 | 18 | 2 | 0 | 97.26% | 5.33% |
| 007 | 30 | 29 | 29 | 26 | 1 | 0 | 98.08% | 2.55% |
| 008 | 24 | 24 | 24 | 23 | 0 | 0 | 99.50% | 0.50% |
| 009 | 28 | 28 | 28 | 26 | 0 | 0 | 99.00% | 1.00% |
| 010 | 36 | 36 | 36 | 34 | 0 | 0 | 99.17% | 0.83% |
| 011 | 37 | 37 | 37 | 35 | 0 | 0 | 99.17% | 0.83% |
| 012 | 26 | 26 | 26 | 26 | 0 | 0 | 100.00% | 0.00% |
| 013 | 32 | 32 | 32 | 32 | 0 | 0 | 100.00% | 0.00% |
| 014 | 30 | 30 | 30 | 27 | 0 | 0 | 98.60% | 1.40% |
| 015 | 26 | 26 | 26 | 25 | 0 | 0 | 99.54% | 0.46% |
| 016 | 30 | 29 | 29 | 29 | 1 | 0 | 100.00% | 0.40% |
| 017 | 31 | 30 | 30 | 27 | 1 | 0 | 98.62% | 2.73% |
| 018 | 28 | 28 | 28 | 27 | 0 | 0 | 99.58% | 0.42% |
| 019 | 27 | 27 | 27 | 25 | 0 | 0 | 99.08% | 0.92% |
| 020 | 32 | 32 | 32 | 31 | 0 | 0 | 99.60% | 0.40% |
| **Tổng** | **758** | **754** | **753** | **711** | **5** | **1** | **99.22%** | **0.97%** |

- **Conditional Character Accuracy:** 99.22% (43 lỗi ký tự / 5.483 ký tự tham chiếu trong các cặp cue đã căn). Không tính ký tự của cue thiếu/thừa.
- **Overall CER:** 0.97% (53 / 5.492 ký tự tham chiếu). 53 lỗi gồm 43 phép sửa trong cue đã căn, 9 ký tự thuộc cue thiếu và 1 ký tự thuộc cue thừa.
- Script căn cue theo overlap thời gian (ngưỡng 40%), sau đó dùng start-time cách nhau tối đa 2 giây để ghép bù. Chỉ số trên là so nguyên văn, không dùng `zhconv`; không nên so trực tiếp với các tỷ lệ lịch sử dùng chuẩn hóa t2s.
- Tổng số cue: 758 tham chiếu, 754 OCR, 753 cặp đã căn, 711 cue khớp nguyên văn, 5 cue thiếu và 1 cue thừa.

## Kiểm chứng trước đó về ảnh caption evidence (001–002)

Các lệnh evidence dưới đây thuộc lượt kiểm chứng trước, tách biệt với lần chạy lại 20 tập S01 ở trên:

```powershell
cargo run --release --example extract_and_merge -- 001 --caption-evidence
cargo run --release --example extract_and_merge -- 002 --caption-evidence
```

| Tập | Kết quả | Caption | PNG | Manifest / warnings | Thời gian |
|---|---|---:|---:|---|---:|
| 001 | Thành công; merge 64 subtitle + 7 caption | 7 | 19 | Có / có, không có cảnh báo | 417.35 giây |
| 002 | Thành công; merge 108 subtitle + 6 caption | 6 | 14 | Có / có, không có cảnh báo | 601.89 giây |

Ảnh được ghi trong `output/caption_evidence/<episode>/run-<id>/`; một ảnh có thể đại diện đồng thời cho first/best hoặc best/last khi cùng chọn một frame. Cả hai lệnh bỏ qua VTT subtitle hợp lệ đã có, chạy lại caption để xuất evidence và hoàn tất bước merge.

Thư mục `output/caption_evidence/` không có trong working copy hiện tại; phần duyệt bên dưới là kết quả lượt kiểm chứng trước, không thể mở lại ảnh trong checkout này.

### Duyệt ảnh

- **001:** Đã xem ảnh first/best/last của cue 6. Manifest ghi `《大女儿-陆紫萱》`, thời gian 71.2–72.5 giây; frame 71.2, 71.4 và 72.4 giây đều cho thấy dòng chữ dọc `大女儿 陆紫萱` trên bia. Nội dung phù hợp cue và ảnh nằm trong khoảng thời gian cue; nghi vấn lệch timestamp của `ep01_06_danver.png` được giải tỏa trên evidence mới.
- **002:** Đã xem ảnh cue 4 tại 202.6, 202.8 và 205.4 giây; chữ dọc tương ứng `《徐清莹弟弟-徐轩宇》`. Sáu cue đều có trong manifest.
- **Phạm vi:** hiện có evidence mới chỉ cho 001–002 (13 cue); 003–020 chưa được tạo ảnh evidence trong lượt kiểm chứng này nên chưa thể kết luận đã duyệt hình ảnh 20/20 tập. Không dùng thống kê caption cũ thay thế cho việc xem ảnh mới.

## Kiểm thử

- `cargo test --workspace`: pass, gồm 89 unit tests và 1 doctest.
- `cargo clippy --workspace --all-targets -- -D warnings`: pass, không có cảnh báo.
- `cargo fmt --all -- --check`: pass.
- `python -m unittest tools.test_compare_subs`: pass (5 tests).
- `python tools\compare_subs.py`: lần chạy trước đã đọc đủ 20 tập; không thể tái chạy trong working copy hiện tại vì thiếu VTT OCR trong `output/`.
