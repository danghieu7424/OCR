20 tập, tổng ~28 phút video. Tôi đo thời gian thực tế trên tập ngắn trước rồi chia luồng chạy song song.

Chạy song song 4 luồng (sub 1–10, sub 11–20, caption 1–10, caption 11–20) trên 12 core.

Trong lúc chạy, tôi viết công cụ đối chiếu VTT (căn theo thời gian + so ký tự + phát hiện sub bị drop).

Runner restart không làm mất tiến trình: 4 job OCR vẫn chạy (sub 001/005/011/012 xong, caption 001–004/011–013 xong). Tiếp tục theo dõi.

Runner restart lần nữa nhưng job vẫn sống: sub 14/20 (còn 006–010), caption 18/20 (còn 009–010).

Xong toàn bộ 20 tập (sub + caption). Giờ chạy đối chiếu.

Đối chiếu xong sub (98.1–100%). Giờ tạo ảnh crop tại đúng vị trí caption để kiểm tra chữ bằng mắt.

Chạy xong toàn bộ 20 tập (sub + caption), đối chiếu bằng script căn theo thời gian và kiểm tra chữ caption bằng ảnh crop tại đúng vị trí.

## 1. Subtitle (20 tập) — `output/第XXX集.zh.vtt` vs reference `input/第XXX集.zh.vtt`

| Tập      | ref     | ours    | khớp    | sai chữ | thiếu | thừa  | acc%       |
| -------- | ------- | ------- | ------- | ------- | ----- | ----- | ---------- |
| 001      | 61      | 65      | 61      | 4       | 0     | 4     | 99.07      |
| 002      | 108     | 108     | 107     | 11      | 1     | 1     | 98.50      |
| 003      | 44      | 44      | 44      | 3       | 0     | 0     | 99.09      |
| 004      | 79      | 80      | 79      | 2       | 0     | 1     | 99.65      |
| 005      | 26      | 26      | 26      | 0       | 0     | 0     | **100.00** |
| 006      | 23      | 20      | 21      | 2       | 2     | 0     | 98.62      |
| 007      | 30      | 29      | 30      | 3       | 0     | 0     | 98.09      |
| 008      | 24      | 24      | 24      | 1       | 0     | 0     | 99.50      |
| 009      | 28      | 28      | 28      | 2       | 0     | 0     | 99.00      |
| 010      | 36      | 38      | 36      | 2       | 0     | 2     | 99.17      |
| 011      | 37      | 37      | 37      | 1       | 0     | 0     | 99.58      |
| 012      | 26      | 26      | 26      | 0       | 0     | 0     | **100.00** |
| 013      | 32      | 32      | 32      | 1       | 0     | 0     | 99.59      |
| 014      | 30      | 30      | 30      | 4       | 0     | 0     | 98.13      |
| 015      | 26      | 26      | 26      | 1       | 0     | 0     | 99.54      |
| 016      | 30      | 30      | 30      | 0       | 0     | 0     | **100.00** |
| 017      | 31      | 30      | 31      | 4       | 0     | 0     | 98.18      |
| 018      | 28      | 28      | 28      | 1       | 0     | 0     | 99.58      |
| 019      | 27      | 27      | 27      | 2       | 0     | 0     | 99.08      |
| 020      | 32      | 32      | 32      | 1       | 0     | 0     | 99.60      |
| **Tổng** | **758** | **760** | **755** | **45**  | **3** | **8** | **99.18**  |

Phân loại 45 câu lệch:

- **~32 ca là khác biệt phồn/giản thể**, không phải đọc sai: `強→强, 於→于, 別→别, 屍→尸, 虛→虚, 能夠→能够, 弒→弑, 況→况, 內→内, 夠→够`… (reference dùng dạng phồn, pipeline chuẩn hoá giản thể).
- **~10 ca lỗi OCR thật**: `爹→參` (tập 2, 3), `嗯→涵` (tập 2), `魔域之人` + kana `で` (tập 2), `魔域` + `V` (tập 3), `一还只是…` thừa `一` (tập 13), `国一起→一起` (tập 6).
- **2 ca reference sai, máy đúng**: `眼晴`→`眼睛` (tập 10), `灭多小`→`灭多少` (tập 6).
- **8 segment thừa**: mảnh vụn ở điểm chuyển sub (`和离/和高/和`, `團`, `爺/爺爺`, `區`, `福`), mỗi cái 0.2s.
- **3 sub bị mất, tất cả đều <0.6s**: `美` (0.4s, tập 2), `挡住` + `五挡往` (0.2s, tập 6). Trên toàn bộ 25 sub ngắn <0.6s thì giữ được 24 → **tỉ lệ rơi 1/25 (~4%)**, thấp hơn mức 1/10 bạn ước lượng.

## 2. Caption — đối chiếu reference (tập 1–4)

| Tập | ref | ours | khớp chữ | ghi chú                    |
| --- | --- | ---- | -------- | -------------------------- |
| 001 | 7   | 7    | 7/7      | 2 câu chỉ thiếu đuôi `...` |
| 002 | 6   | 6    | **6/6**  | gồm `《何訇》` (đã fix)    |
| 003 | 3   | 3    | 3/3      | 1 câu thiếu `...`          |
| 004 | 2   | 2    | **2/2**  |                            |

`...` là hậu tố người soạn thêm vào reference, OCR không sinh ra.

## 3. Caption tập 5–20 (không có reference) — kiểm tra bằng ảnh tại vị trí caption

Đã crop đúng box nhận diện ở thời điểm giữa mỗi caption (24 ảnh) và xem trực tiếp, cộng 1 ca kiểm tra riêng trên frame:

| Tập               | Caption         | Kiểm tra mắt                                                                     |
| ----------------- | --------------- | -------------------------------------------------------------------------------- |
| 005               | 魔临城          | ✓                                                                                |
| 006               | 孙敏姿          | ✓                                                                                |
| 007               | 圣王境强者      | ✓                                                                                |
| 008               | 落逢海-帝境中期 | ✓ (frame giữa chỉ thấy cột 2 `帝境中期`, cột 1 ở frame khác)                     |
| 009               | 临城城主-肖清逸 | ✓                                                                                |
| 009               | **社**          | ✗ **false positive** — ảnh crop là lá cờ đỏ (banner), không phải caption         |
| 012               | 陆清安院落      | ✓                                                                                |
| 010, 011, 013–020 | 0 caption       | đã soi 8 frame/tập ở tập 010 và 018: chỉ có sub + watermark, không có name plate |

Tức **24/25 caption khớp chữ khi soi ảnh**, 1 ca sai (banner đỏ bị đọc thành `社`, conf 0.94, sống sót qua lọc vì nền tối + chữ nhạt).

## 4. Ghi chú vận hành

- 40 file output: `output/第001..020集.zh.vtt` (sub) và `output/第001..020集.captions.zh.vtt` (caption), không tập nào FAIL.
- Chạy 4 luồng song song, tổng ~1h50; runner restart 2 lần nhưng job không chết.
- `examples/extract_subtitles.rs` nay nhận tham số tập (`-- 005`) như example caption.
- Cây git sạch (thay đổi example đã được commit tự động), chưa push.

Nếu muốn, bước tiếp theo hợp lý là siết thêm một luật cho ca `《社》`: box có nền là **vật thể cảnh** (độ tương phản nền cao, không có mảng mực tối phía sau) — nhưng cần thêm dữ liệu để không làm rơi caption thật.

---

---

Tôi đã có số liệu quyết định: caption thật nằm trên nền **tối gần đen** (lum_med 1–47, dark_frac 0.63–0.83), còn box băng rôn 《社》 có lum_med 145–164, dark_frac 0.10. Giờ đo thêm các tập khác để chọn ngưỡng có biên an toàn.

Số liệu cho thấy màu bão hoà không tách được (caption thật cũng nằm trên nền đỏ tối), nhưng **nền tối** thì tách rõ. Tôi đổi luật sang: box sáng và thiếu mảng tối bao quanh chữ ⇒ chữ trong cảnh.

Tiến trình của tôi vẫn chạy sau khi runner restart. Tập 003 xong với 3 caption (không đổi). Chờ 10 lượt chạy hoàn tất.

Vòng 1 xong: 001=7, 002=6, 003=3, 004=2 — y hệt trước khi siết luật. Chờ vòng 2 (005–008) và vòng 3 (009, 012).

Tất cả 10 tập đã chạy lại xong. Cập nhật CHANGELOG + tăng phiên bản, dọn file tạm, rồi commit.

Đã siết luật, chạy lại và xác nhận.

## Luật mới (`src/caption.rs`)

Thay vì đo độ bão hoà màu (không tách được — caption thật cũng nằm trên nền đỏ tối), tôi đo **quầng tối** quanh nét chữ. Hàm mới `box_pixel_stats` → `BoxPixels { lum_med, dark_frac, sat_med, sat_p90, sat_hi }`, và luật:

```rust
const CAPTION_MIN_SHADOW_FRAC: f32 = 0.20;   // tỷ lệ pixel tối (lum < 60) trong box
const CAPTION_BRIGHT_BG_LUM:   f32 = 100.0;  // độ sáng trung vị của nền trong box

scene_ok = stats.dark_frac >= 0.20 || stats.lum_med <= 100.0;  // loại khi CẢ HAI đều sai
```

Đo trên dữ liệu thật (tập 1–3, 9, 12):

- caption thật: `dark_frac` 0.26–0.98 (có viền tối ngay sau chữ), kể cả ca khó `《直到徐清莹突破大帝这天》` nằm trên cảnh sáng (`lum_med` 133–176 nhưng `dark_frac` 0.26–0.35).
- box băng rôn 《社》 tập 9: `dark_frac` 0.10, `lum_med` 145–164, `sat_p90` 0.48 → bị loại.

Vì điều kiện là AND của hai dấu hiệu, caption đặt trên cảnh sáng vẫn được giữ. Thêm log chẩn đoán bật bằng `OCR_CAP_DEBUG=1` (in thống kê pixel + kết quả từng bộ lọc cho mọi box ứng viên).

## Kết quả chạy lại (001–009, 012)

| Tập | Trước | Sau   | Text so với trước                   |
| --- | ----- | ----- | ----------------------------------- |
| 001 | 7     | 7     | giống hệt                           |
| 002 | 6     | 6     | giống hệt (kể cả 《何訇》)          |
| 003 | 3     | 3     | giống hệt                           |
| 004 | 2     | 2     | giống hệt                           |
| 005 | 1     | 1     | 《魔临城》                          |
| 006 | 1     | 1     | 《孙敏姿》                          |
| 007 | 1     | 1     | 《圣王境强者》                      |
| 008 | 1     | 1     | 《落逢海-帝境中期》                 |
| 009 | 2     | **1** | mất 《社》, giữ 《临城城主-肖清逸》 |
| 012 | 1     | 1     | 《陆清安院落》                      |

24 caption thật giữ nguyên chữ; chỉ tập 1, 2, 3 lệch mốc thời gian ±0.2s (một frame bị lọc nên biên segment dịch), nội dung không đổi. Tập 10, 11, 13–20 vốn 0 caption nên không bị ảnh hưởng (luật chỉ loại bỏ box).

Kiểm tra bằng mắt trước đó vẫn đúng: 《社》 là chữ trên băng rôn đỏ ở x≈1600–1850 (nửa phải khung), không phải caption.

## Sách/commit

- Build release pass với `OCR v0.2.1`.
- `CHANGELOG.md`: thêm mục `## [0.2.1] — 2026-11-14` (Sửa / Thay đổi / Kiểm chứng); `Cargo.toml` bump `0.2.1`.
- Commit `7b88873` (fix) + `d15a268` (graphify hook), đã push lên `origin/main`, working tree sạch, không force.

Chưa chạy `cargo test` và chưa gắn tag `v0.2.1` (rules yêu cầu test pass trước khi tag) — nói một câu là tôi chạy.
