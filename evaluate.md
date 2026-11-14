# Kết quả kiểm chứng OCR

Ngày chạy: 2026-11-14. Báo cáo này thay thế các số liệu lịch sử cũ; các chỉ số bên dưới được tính lại trên VTT hiện có bằng `tools/compare_subs.py`, không chuẩn hóa phồn/giản thể (`--norm none`).

## Subtitle: 20 tập

| Tập | Ref cues | OCR cues | Cặp căn | Cue đúng | Thiếu | Thừa | Conditional Character Accuracy | Overall CER |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| 001 | 61 | 64 | 61 | 57 | 0 | 3 | 99.07% | 2.10% |
| 002 | 108 | 108 | 107 | 97 | 1 | 1 | 98.64% | 1.63% |
| 003 | 44 | 44 | 44 | 41 | 0 | 0 | 99.09% | 0.91% |
| 004 | 79 | 80 | 79 | 77 | 0 | 1 | 99.65% | 0.53% |
| 005 | 26 | 26 | 26 | 26 | 0 | 0 | 100.00% | 0.00% |
| 006 | 23 | 21 | 21 | 18 | 2 | 0 | 97.93% | 5.33% |
| 007 | 30 | 29 | 29 | 26 | 1 | 0 | 98.08% | 2.55% |
| 008 | 24 | 24 | 24 | 23 | 0 | 0 | 99.50% | 0.50% |
| 009 | 28 | 28 | 28 | 26 | 0 | 0 | 99.00% | 1.00% |
| 010 | 36 | 38 | 36 | 34 | 0 | 2 | 99.17% | 2.08% |
| 011 | 37 | 37 | 37 | 36 | 0 | 0 | 99.58% | 0.42% |
| 012 | 26 | 26 | 26 | 26 | 0 | 0 | 100.00% | 0.00% |
| 013 | 32 | 32 | 32 | 31 | 0 | 0 | 99.59% | 0.41% |
| 014 | 30 | 30 | 30 | 26 | 0 | 0 | 98.13% | 1.87% |
| 015 | 26 | 26 | 26 | 25 | 0 | 0 | 99.54% | 0.46% |
| 016 | 30 | 30 | 30 | 30 | 0 | 0 | 100.00% | 0.00% |
| 017 | 31 | 30 | 30 | 27 | 1 | 0 | 98.62% | 2.73% |
| 018 | 28 | 28 | 28 | 27 | 0 | 0 | 99.58% | 0.42% |
| 019 | 27 | 27 | 27 | 25 | 0 | 0 | 99.08% | 0.92% |
| 020 | 32 | 32 | 32 | 31 | 0 | 0 | 99.60% | 0.40% |
| **Tổng** | **758** | **760** | **753** | **709** | **5** | **7** | **99.20%** | **1.17%** |

- **Conditional Character Accuracy:** 99.20% (44 lỗi ký tự / 5.482 ký tự tham chiếu trong các cặp cue đã căn). Không tính ký tự của cue thiếu/thừa.
- **Overall CER:** 1.17% (64 / 5.492 ký tự tham chiếu). 64 lỗi gồm 44 phép sửa trong cue đã căn, 10 ký tự thuộc cue thiếu và 10 ký tự thuộc cue thừa.
- Script căn cue theo overlap thời gian (ngưỡng 40%), sau đó dùng start-time cách nhau tối đa 2 giây để ghép bù. Chỉ số trên là so nguyên văn, không dùng `zhconv`; không nên so trực tiếp với các tỷ lệ lịch sử dùng chuẩn hóa t2s.

## Kiểm chứng lệnh trích xuất và ảnh caption evidence

Đã chạy thành công:

```powershell
cargo run --release --example extract_and_merge -- 001 --caption-evidence
cargo run --release --example extract_and_merge -- 002 --caption-evidence
```

| Tập | Kết quả | Caption | PNG | Manifest / warnings | Thời gian |
|---|---|---:|---:|---|---:|
| 001 | Thành công; merge 64 subtitle + 7 caption | 7 | 19 | Có / có, không có cảnh báo | 417.35 giây |
| 002 | Thành công; merge 108 subtitle + 6 caption | 6 | 14 | Có / có, không có cảnh báo | 601.89 giây |

Ảnh được ghi trong `output/caption_evidence/<episode>/run-<id>/`; một ảnh có thể đại diện đồng thời cho first/best hoặc best/last khi cùng chọn một frame. Cả hai lệnh bỏ qua VTT subtitle hợp lệ đã có, chạy lại caption để xuất evidence và hoàn tất bước merge.

### Duyệt ảnh

- **001:** Đã xem ảnh first/best/last của cue 6. Manifest ghi `《大女儿-陆紫萱》`, thời gian 71.2–72.5 giây; frame 71.2, 71.4 và 72.4 giây đều cho thấy dòng chữ dọc `大女儿 陆紫萱` trên bia. Nội dung phù hợp cue và ảnh nằm trong khoảng thời gian cue; nghi vấn lệch timestamp của `ep01_06_danver.png` được giải tỏa trên evidence mới.
- **002:** Đã xem ảnh cue 4 tại 202.6, 202.8 và 205.4 giây; chữ dọc tương ứng `《徐清莹弟弟-徐轩宇》`. Sáu cue đều có trong manifest.
- **Phạm vi:** hiện có evidence mới chỉ cho 001–002 (13 cue); 003–020 chưa được tạo ảnh evidence trong lượt kiểm chứng này nên chưa thể kết luận đã duyệt hình ảnh 20/20 tập. Không dùng thống kê caption cũ thay thế cho việc xem ảnh mới.

## Kiểm thử

- `cargo test --workspace`: pass, gồm 54 unit tests và 1 doctest.
- `python -m unittest tools.test_compare_subs`: pass (2 tests).
- `python tools\compare_subs.py`: pass, đọc đủ 20 tập và in kết quả Unicode trên Windows.
