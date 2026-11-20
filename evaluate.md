# Kết quả kiểm chứng OCR

Ngày chạy: 2026-11-15. Bảng dưới đây ghi lại lần chạy lại 20 tập S01 sau các thay đổi OCR, so sánh bằng `tools/compare_subs.py` trên VTT subtitle OCR thô, không chuẩn hóa phồn/giản thể (`--norm none`). Ground truth trong `input/` được giữ nguyên; dữ liệu chạy lại trong `scratch/` đã được dọn sau khi đối chiếu.

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

## S02 pipeline acceptance: 7 tập gốc

Đã chạy `ocr-cli --force --caption-evidence` trên bản sao byte-identical (SHA-256 khớp) của `S02E001`, `S02E003`, `S02E005`, `S02E006`, `S02E007`, `S02E017` và `S02E019` từ `in/`. Ground truth không bị sửa. Log, merged VTT, hai VTT OCR thô và crop evidence được giữ tại `output/s02-acceptance-20261115/`.

| Tập | Kết quả merge | Caption |
|---|---:|---:|
| S02E001 | 58 cues (52 subtitle + 6 caption) | 6 |
| S02E003 | 52 cues (52 subtitle + 0 caption) | 0 |
| S02E005 | 15 cues (15 subtitle + 0 caption) | 0 |
| S02E006 | 27 cues (26 subtitle + 1 caption) | 1 |
| S02E007 | 27 cues (27 subtitle + 0 caption) | 0 |
| S02E017 | 28 cues (26 subtitle + 2 caption) | 2 |
| S02E019 | 22 cues (21 subtitle + 1 caption) | 1 |

### Kiểm tra các lỗi đã báo

- **S02E001:** crop evidence xác nhận `《雪天妃》` và `《陆清安-妖魔体状态》` là hai cue/crop riêng ở 9.8–11.5s; `《骆丹茲》` và `《肖清逸》` cũng tách riêng ở 116.2–117.5s. Mốc 197s không thuộc thời lượng video.
- **S02E005:** `武司` không còn; cue là `永夜魔帝`, không dính `富`.
- **S02E006:** cụm rác `富龙麗龍龍黑龙` không còn; cue tại 27.85–28.65s là `黑龙`.
- **S02E007:** `龙流` và hậu tố `光` không còn; giữ đủ `一路上所有虚空都得被你破坏`.
- **S02E017:** crop xác nhận caption `《何訇》` tại 5.6–7.1s đã được khôi phục.
- **S02E019:** crop xác nhận `《骆丹茲》` tại 32.8–34.3s; false positive `《区》` không có trong VTT caption của lượt chạy mới. Đã kiểm tra thêm clip debug, các box `区` nhỏ/confidence 0.41–0.66 bị loại khỏi track cứu.

## Kiểm chứng trước đó về ảnh caption evidence S01 (001–002)

Các lệnh và phạm vi dưới đây thuộc lượt kiểm chứng ảnh S01 trước đó, tách biệt với lần chạy lại 20 tập S01 và nghiệm thu 7 tập S02 ở trên:

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
- **Phạm vi của lượt S01 trước đó:** evidence chỉ được tạo cho 001–002 (13 cue); 003–020 không có ảnh trong lượt đó. Không dùng thống kê caption cũ thay thế cho việc xem ảnh mới.

## Kiểm chứng gán nhãn nhân vật (Speaker Attribution / Active Speaker Detection)

Ngày chạy: 2026-11-20. Đối chiếu bằng `crates/light_asd/examples/attribute_speakers.rs` trên hai tập ground truth `final/第001集_final.zh.diarized.vtt` và `final/第002集_final.zh.diarized.vtt`, định dạng thẻ chuẩn W3C WebVTT `<v Nhân_vật>câu thoại</v>`. Nội dung chữ OCR và caption được giữ nguyên 100%.

| Tập | Số cue phụ đề tham chiếu | Cặp căn | Đúng nhân vật | Độ chính xác gán nhãn |
|---|---:|---:|---:|---:|
| 第001集 | 61 | 61 | 59 | 96.72% |
| 第002集 | 108 | 107 | 99 | 91.67% |
| **Tổng** | **169** | **168** | **158** | **93.49%** |

- Cả hai tập đều vượt ngưỡng yêu cầu > 90.0% (Tập 1 đạt 96.72%, Tập 2 đạt 91.67%, tổng thể đạt 93.49%).
- Toàn bộ nội dung OCR và caption `《...》` (line:20%) được giữ nguyên vẹn 100%, bảo toàn độ chính xác ký tự 99.22%.

## Kiểm thử

- `cargo test --workspace`: pass, gồm 90 unit tests và 1 doctest.
- `cargo clippy --workspace --all-targets -- -D warnings`: pass, không có cảnh báo.
- `cargo fmt --all -- --check`: pass.
- `python -m unittest tools.test_compare_subs`: pass (5 tests).
- `python tools\compare_subs.py`: đã đối chiếu đủ 20 tập S01 với `--norm none`; VTT tạm được dọn sau khi ghi số liệu ở trên.
- `ocr-cli --force --caption-evidence`: hoàn tất đủ 7/7 video S02 gốc; merged/raw VTT, crop evidence, hai frame kiểm tra false positive và log được giữ tại `output/s02-acceptance-20261115/`.
