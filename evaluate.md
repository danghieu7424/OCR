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

---

---

Phát hiện vài cặp bị ghép lệch (ví dụ `美` ↔ `孩儿对不起您`), nên tôi cải thiện thuật toán căn trước khi kết luận.

**Kết luận: nên chuẩn hoá, nhưng chỉ ở bước chấm điểm — không thêm vào pipeline.**

## Vì sao

Quét toàn bộ 20 file `output/*.zh.vtt` (sub): chỉ có **1 ký tự phồn thể** trong hơn 5.480 ký tự (團, một mảnh vụn ở tập 1). Tức pipeline đã xuất giản thể gần như tuyệt đối (vocab của model là giản thể). Ngược lại, reference dùng phồn thể lẫn lộn. Nên chuẩn hoá phồn→giản ở khâu so sánh, không phải ở khâu OCR.

## So sánh trước/sau trên 20 tập

| Cách chấm                 | Khớp | Đúng nguyên văn | Lệch | Lỗi ký tự | Accuracy   |
| ------------------------- | ---- | --------------- | ---- | --------- | ---------- |
| Không chuẩn hoá           | 752  | 708             | 44   | 44/5480   | **99.20%** |
| Chuẩn hoá t2s (phồn→giản) | 752  | 742             | 10   | 10/5480   | **99.82%** |
| Chuẩn hoá s2t (giản→phồn) | 752  | 741             | 11   | 11/5480   | 99.80%     |

Trong 44 câu lệch khi chấm thô, **34 câu chỉ khác biến thể chữ** (`強/强`, `於/于`, `別/别`, `屍/尸`, `虛/虚`, `能夠/能够`, `弒/弑`, `況/况`, `內/内`, `夠/够`, `團/团`, `爺/爷`, `區/区`). t2s và s2t cho kết quả gần như trùng nhau, nhưng **t2s là hướng đúng** vì s2t có ánh xạ một-nhiều (干→乾/幹, 发→發/髮) có thể tạo lỗi giả.

### Theo tập (accuracy thô → sau t2s)

| Tập | Thô    | t2s        | Tập | Thô    | t2s        |
| --- | ------ | ---------- | --- | ------ | ---------- |
| 001 | 99.07  | **100.00** | 011 | 99.58  | **100.00** |
| 002 | 98.50  | 99.46      | 012 | 100.00 | 100.00     |
| 003 | 99.09  | 99.39      | 013 | 99.59  | 99.59      |
| 004 | 99.65  | **100.00** | 014 | 98.13  | **100.00** |
| 005 | 100.00 | 100.00     | 015 | 99.54  | **100.00** |
| 006 | 98.60  | 98.60      | 016 | 100.00 | 100.00     |
| 007 | 98.08  | **100.00** | 017 | 98.62  | **100.00** |
| 008 | 99.50  | **100.00** | 018 | 99.58  | **100.00** |
| 009 | 99.00  | **100.00** | 019 | 99.08  | **100.00** |
| 010 | 99.17  | 99.58      | 020 | 99.60  | **100.00** |

Chỉ 4 tập còn lệch sau chuẩn hoá: 002 (4), 003 (2), 006 (2), 010 (1), 013 (1).

## 10 lỗi thật còn lại (sau t2s)

Trong đó **6 lỗi do OCR**, **4 chỗ reference sai mà máy đúng**:

| Tập | ref                | OCR                  | Loại                 |
| --- | ------------------ | -------------------- | -------------------- |
| 002 | 魔域之人           | 魔域之人**で**       | OCR thêm kana        |
| 002 | 嗯                 | 涵                   | OCR sai chữ          |
| 003 | 那可是魔域         | 那可是魔域**V**      | OCR thêm ký tự Latin |
| 003 | 爹以前的武器给你   | **参**以前的武器给你 | OCR 爹→参            |
| 006 | 国一起             | 一起                 | OCR rơi chữ 国       |
| 013 | 还只是比你强一点点 | **一**还只是…        | OCR thêm 一          |
| 002 | 娘亲爹**參**       | 娘亲爹爹             | reference sai        |
| 002 | 自**已**           | 自己                 | reference sai        |
| 006 | 灭多**小**         | 灭多少               | reference sai        |
| 010 | 眼**晴**           | 眼睛                 | reference sai        |

Nên chất lượng OCR thật ≈ **6 lỗi / 5480 ký tự = 99.89%**, và điểm 99.82% sau chuẩn hoá là cận dưới hợp lý (bị 4 lỗi reference kéo xuống).

Ngoài lỗi ký tự, phần còn lại là cấu trúc, không liên quan phồn/giản: **6 câu thiếu** (đều 0.2–1.6s: `美`, `挡住`, `五挡往`, `啊啊`, `滚`, `别说了`) và **8 segment thừa** (mảnh vụn ở điểm chuyển sub: `團`, `和离/和高/和`, `区`, `福`, `爷爷/爷`).

## Đã làm

- `tools/compare_subs.py`: căn câu theo overlap giảm dần **toàn cục** rồi bù bằng start gần nhất (bản cũ ghép tuần tự nên dây chuyền lệch khi mất câu ngắn, ví dụ `美` ↔ `孩儿对不起您`), so ký tự Levenshtein, cờ `--norm none|t2s|s2t`, `--show <tập>` in chi tiết. Đã dùng `zhconv` cài tạm (`pip install --target scratch/pydeps`) rồi xoá — không phải dependency dự án; script chỉ import khi dùng `t2s`/`s2t`.
- `CHANGELOG.md`: mục `## [Unreleased] → ### Thêm` ghi tool này kèm số đo.
- Commit `d3f33d9` + `8944882` (dọn zhconv tạm) + `dbf2182` (graphify), đã push; cây làm việc sạch.

Gợi ý bước tiếp (chưa làm): nếu muốn con số sát hơn nữa, sửa 4 lỗi reference trong bản so sánh hoặc tách riêng "reference-lỗi" — nhưng như vậy là sửa dữ liệu chuẩn, nên để bạn quyết.

---

---

Đã test 2 phương án trên một vài tập; **cả hai đều không cứu được sub nào mà chỉ thêm nhiễu**, nên tôi giữ nguyên luật hiện tại.

## Chẩn đoán trước: "sub bị mất" phần lớn không phải do luật 2 frame

Bật `OCR_SUB_DEBUG=1` để xem phân đoạn thô:

| Ref báo thiếu         | Thực tế                                                                                                                                                         |
| --------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| ep7 `滚` 17.05–17.35  | ref tách đôi cùng một caption: `17.050–17.350 滚` + `17.351–17.550 滾`. OCR có `滚` 16.95–17.65 (7 frame) và gộp đúng ✔                                         |
| ep17 `别说了` 67.05   | ref cũng tách đôi: `別说了` 67.05–67.25 + `别说了` 67.25–67.75. OCR có `别说了` 66.95–67.75 ✔                                                                   |
| ep6 `挡住` / `五挡往` | ref tách 2 mảnh 0.2s của **một** caption; OCR thô mỗi frame ra rác (`挡一`, `挡`, `星 挡住是本朱`) — lỗi nhận diện trên nền bùa chú rối, không phải thiếu frame |
| ep2 `美` 50.35–50.75  | soi frame 49.9 / 50.1 / 50.35 / 50.55 / 50.75 / 51.0: **không có chữ nào trên màn hình** → dòng ref này là rác                                                  |

Tức chỉ còn **đúng 1 ca thật**: `挡住` ở ep6, và nó hỏng vì OCR đọc sai trên nền rối.

## Phương án A — giữ câu 1–2 chữ Hán hiện 1 frame (test tập 001–005)

Thêm: `if seg.end_frame < seg.start_frame + 1 && !short_cjk` (short_cjk = ≤2 chữ Hán).

| Tập | Số câu trước → sau | Câu thêm                              |
| --- | ------------------ | ------------------------------------- |
| 001 | 65 → 65            | —                                     |
| 002 | 108 → 111          | `海` (0.1s), `夏` (0.1s), `艾` (0.1s) |
| 003 | 44 → 45            | `華` (0.1s)                           |
| 004 | 80 → 80            | —                                     |
| 005 | 26 → 27            | `污` (0.1s)                           |

**+5 câu, 0 câu đúng** — toàn ký tự rác 0.1s. → loại.

## Phương án B — hạ dt (10 → 12 fps, test ep2 và ep6)

| Tập | 10 fps | 12 fps | Khác biệt                                                                                               |
| --- | ------ | ------ | ------------------------------------------------------------------------------------------------------- |
| 002 | 108    | 110    | sửa được `魔域之人で` → `魔域之人` (hết kana rác) nhưng thêm `工`, `国` (2 mảnh rác); `美` vẫn không có |
| 006 | 20     | 20     | `挡住` **vẫn mất**                                                                                      |

→ 12 fps sửa 1 lỗi chữ, thêm 2 nhiễu, không cứu được câu nào; thời gian chạy +20% (ep2: 216s video → ~11 phút). Không đáng.

## Kết luận

Giữ nguyên: 10 fps, tối thiểu 2 frame liên tiếp. Nút thắt thật sự là **độ ổn định nhận diện theo thời gian** (ep6 nền bùa chú), không phải số frame tối thiểu — hướng đúng nếu muốn cứu `挡住` là gộp phiếu trên các frame lân cận theo `is_same_subtitle` (chỉ nhận chuỗi con xuất hiện ≥2 frame trong cửa sổ ±0.5s), chứ không hạ ngưỡng frame (sẽ kéo rác vào).

## Trạng thái

- Code trả lại luật cũ; chỉ còn 2 knob để đo lại nhanh: `OCR_SUB_DEBUG=1` (in phân đoạn thô + lý do loại) và `OCR_SUB_FPS=12` (mặc định 10).
- Output 20 tập đã trả về bản 10 fps (ep2, ep6 đã `cmp` khớp bản gốc).
- `CHANGELOG.md` mục `[Unreleased] → ### Thêm` có 1 dòng cho 2 knob này.
- Commit `23c5cb1` + `9b1faec` (graphify), đã push, cây sạch.

Chưa kiểm chứng: chỉ đo trên 5 tập (A) và 2 tập (B) như bạn yêu cầu; chưa chạy lại toàn bộ 20 tập với các phương án đó.

---

---

Cần biết thời gian rơi vào đâu trước khi tối ưu. Thêm đo thời gian det/rec mỗi frame.

Đã đọc `evaluate.md`, đo lại đường ống và tối ưu cả tốc độ lẫn chất lượng (test trên tập 002/005/006).

## Đo trước khi sửa (tập 006, 719 frame)

| Thành phần  | ms/frame  | Ghi chú                   |
| ----------- | --------- | ------------------------- |
| DBNet (det) | **213,2** | chiếm 87% thời gian       |
| Rec phụ đề  | 32,8      |                           |
| Tổng        | ~248      | wall 178 s cho video 69 s |

Nghĩa là nút thắt nằm ở detect: ảnh ROI 1920×162 được đưa vào DBNet ở giới hạn cạnh dài 960 (mặc định của crate), tốn 213 ms/frame.

## Đã tối ưu

**1. Hạ cạnh dài DBNet 960 → 640** (`create_det_engine_with_limit`, đổi lại được bằng `OCR_SUB_DET_LIMIT`)
| Giới hạn | det ms/frame | Wall (ep006) | Chất lượng |
|---|---|---|---|
| 960 (cũ) | 213,2 | 178 s | ep002 98,50 % · ep005 100 % · ep006 20 câu |
| **640 (mới)** | **125–145** | **132 s** | ep002 **98,64 %** · ep005 **giống hệt byte-for-byte** · ep006 21 câu |
| 320 (loại) | 71,4 | 71 s | sai nhiều câu (`可是我如今` lặp, `这是1`, `一—一起`) → không dùng |

→ **nhanh hơn ~1,4× (≈35 % thời gian)**, chất lượng bằng hoặc tốt hơn.

**2. Dọn rác nhận diện** (`sanitize_ocr_text`)

- Bỏ kana: `魔域之人で` → `魔域之人` (lỗi thật trong báo cáo cũ).
- Bỏ một ký tự Latin lẻ dính cuối: `魔域V` → `魔域`.

**3. Bỏ mảnh vụn ở điểm chuyển sub**
Câu 1–2 chữ, ≤0,3 s, nằm sát câu khác và là tập con của câu đó (`和` cạnh `和离`, `爷` cạnh `爷爷`) → loại. Hai segment thừa đã được xử lý trong lần chạy này mà không mất câu thật nào.

**4. Log chẩn đoán mới**: `OCR_SUB_DEBUG=1` in `[TIMING] frames | det ms/frame | rec ms/frame` (trước đó không biết thời gian rơi vào đâu).

## Kết quả chất lượng sau khi sửa (so với reference)

| Tập | Trước                  | Sau                  | Thay đổi                                                                               |
| --- | ---------------------- | -------------------- | -------------------------------------------------------------------------------------- |
| 002 | 98,50 % · 11 lỗi ký tự | **98,64 % · 10 lỗi** | hết `で`, `V`; vẫn còn `涵` vs `嗯`, `参` vs `爹` và 4 chỗ reference sai               |
| 005 | 100 % · 26 câu         | **100 % · 26 câu**   | VTT giống hệt từng byte                                                                |
| 006 | 98,60 % · thiếu `挡住` | 21 câu, có `挡`      | lấy lại được mảnh của `挡住`; `啊啊` (1,6 s) vẫn mất do nhận diện sai trên nền bùa chú |

## Ghi chú

- Detector vẫn giữ 960 cho pipeline **caption** (crop nhỏ, chữ mảnh) — thay đổi này chỉ áp cho phụ đề.
- `OCR_SUB_DET_LIMIT=960` để quay lại hành vi cũ nếu gặp tập có sub chữ nhỏ.
- Chỉ đo trên 3 tập theo yêu cầu "test một vài tập"; 17 tập còn lại trong `output/` vẫn là bản 960 (chất lượng tương đương, chỉ chậm hơn).
- Đã build release `OCR v0.3.0`, cập nhật `CHANGELOG.md` (mục `[Unreleased] → ### Thay đổi`), commit `15e77ee` + `827d16f` (graphify) và push. `evaluate.md` đang dirty do công cụ tự nối nhật ký — tôi không đụng vào.
