# Hướng dẫn sử dụng `ocr-cli` trên Windows

`ocr-cli` trích xuất subtitle và caption bằng OCR thuần, rồi hợp nhất thành một file WebVTT. Chương trình không tự sửa nội dung OCR.

## 1. Cấu trúc gói

Thư mục hiện tại được đóng gói như sau:

```text
ocr-cli\
  ocr-cli.exe
  DirectML.dll
  models\
    ocr\
      det.onnx
      rec.onnx
      keys.txt
```

Giữ `DirectML.dll` cạnh `ocr-cli.exe` và giữ nguyên cấu trúc `models\ocr\`. `ffmpeg` và `ffprobe` phải được cài riêng và có trong `PATH`.

Nếu cần build lại từ mã nguồn, chạy từ thư mục gốc repository:

```powershell
cargo build --release --bin ocr-cli
```

Executable được tạo tại `target\release\ocr-cli.exe`. Cargo không tự chép model vào executable; hãy chép `ocr-cli.exe`, `DirectML.dll` và thư mục `models` theo cấu trúc ở trên.

## 2. Chạy một video

Mở PowerShell trong thư mục chứa gói:

```powershell
.\ocr-cli.exe --input "C:\Videos\episode.mp4" --output "C:\Videos\output" --models ".\models"
```

Có thể dùng cờ ngắn:

```powershell
.\ocr-cli.exe -i "C:\Videos\episode.mp4" -o "C:\Videos\output" --models ".\models"
```

Hoặc dùng tham số input positional để tương thích với cách gọi cũ:

```powershell
.\ocr-cli.exe "C:\Videos\episode.mp4" -o "C:\Videos\output" --models ".\models"
```

## 3. Chạy một folder video

Folder được quét **không đệ quy**. Các định dạng được nhận: `.mp4`, `.mkv`, `.avi`, `.mov`, `.ts`, `.flv`, `.webm`.

```powershell
.\ocr-cli.exe --input "C:\Videos" --output "C:\Videos\merged" --models ".\models"
```

Mỗi video tạo một thư mục `.tmp_<tên-video>` cạnh file đầu vào, chứa hai WebVTT OCR thô:

```text
C:\Videos\
  episode.mp4
  .tmp_episode\
    state.json
    subtitles.zh.vtt
    captions.zh.vtt
```

File merge được ghi trong output với tên `<tên-video>.merged.zh.vtt`. Nếu nhiều video trong folder có cùng tên gốc, CLI báo lỗi để tránh ghi đè VTT và thư mục tạm.

Trong lúc OCR, subtitle và caption được trích xuất song song. Hai thanh `[SUB]` và `[CAP]` có màu riêng: thanh tiến trình xanh dương; nhãn đang chạy xanh cyan; hoàn tất xanh lá; cần duyệt/cảnh báo cam; lỗi đỏ. Khi bước trích xuất thành công, phần trăm được chốt ở 100%.

### Tiếp tục sau khi bị gián đoạn

Mỗi video lưu checkpoint tại `.tmp_<tên-video>\state.json`. Lần chạy sau, CLI kiểm tra trạng thái từng bước subtitle, caption và merge, rồi bỏ qua bước đã hoàn tất và còn đầu ra hợp lệ. Bước chưa xong sẽ tiếp tục; file video, model hoặc cấu hình OCR thay đổi sẽ làm bước liên quan chạy lại. Thay đổi riêng `--subtitle-line`/`--caption-line` chỉ chạy lại bước merge.

Thêm `--force` để chạy lại toàn bộ subtitle, caption và merge, kể cả khi checkpoint cho biết đã hoàn tất:

```powershell
.\ocr-cli.exe -i "C:\Videos\episode.mp4" -o "C:\Videos\output" `
  --models ".\models" --caption-evidence --force
```

Khi dùng `--force`, raw VTT được tạo lại; nếu bật `--caption-evidence`, thư mục evidence của từng bộ OCR cũng được tạo lại.

## 4. Ảnh evidence

Thêm `--caption-evidence` để tạo ảnh first/best/last cho cue subtitle và caption:

```powershell
.\ocr-cli.exe -i "C:\Videos\episode.mp4" -o "C:\Videos\output" `
  --models ".\models" --caption-evidence
```

Ảnh và manifest nằm trong `.tmp_<tên-video>\subtitle_evidence\` và `.tmp_<tên-video>\caption_evidence\`. Evidence subtitle là ảnh crop theo ROI subtitle tại frame đầu, giữa và cuối cue; evidence caption là crop theo vùng detector caption. Không bật cờ này thì CLI không xuất evidence mới.

## 5. Điều chỉnh ROI và vị trí `line`

ROI có thứ tự `Y_MIN,Y_MAX,X_MIN,X_MAX`; mỗi giá trị là tỷ lệ từ `0` đến `1`.

- Subtitle mặc định: `0.85,1.0,0.05,0.95`.
- Caption mặc định: `0.10,0.80,0,1`.

Ví dụ điều chỉnh cả hai ROI và vị trí WebVTT:

```powershell
.\ocr-cli.exe -i "C:\Videos\episode.mp4" -o "C:\Videos\output" `
  --models ".\models" `
  --subtitle-roi 0.85,1.0,0.05,0.95 `
  --caption-roi 0.10,0.80,0,1 `
  --subtitle-line 85% `
  --caption-line 20%
```

```powershell
./ocr-cli/ocr-cli.exe -i input -o input  --caption-evidence `
  --subtitle-roi 0.85,1.0,0.05,0.95 `
  --caption-roi 0.10,0.80,0,1 `
  --subtitle-line 90% `
  --caption-line 20%
```

`--subtitle-line` và `--caption-line` chỉ ghi đè cue setting `line` trong file merge; có thể truyền `85%` hoặc `line:85%`. Các setting khác như `position` và `align` được giữ nguyên. Hai file raw VTT trong `.tmp_<tên-video>` không bị thay đổi bởi override này.

## 6. Xem trợ giúp

```powershell
.\ocr-cli.exe --help
```